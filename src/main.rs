mod agents;
mod clipboard;
mod cloudflared;
mod live;
mod os;
mod proxy;
mod share;
mod state;
mod tui;
mod tunnel;
mod widget;

use std::io::{IsTerminal, Write};
use std::sync::mpsc;
use std::time::Duration;

use clap::{Parser, Subcommand};
use tokio::sync::watch;

use share::Event;

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);
const EXIT_USAGE: i32 = 2;

const AFTER_HELP: &str = r#"Examples:
  bunflared 5173                  share one app
  bunflared 5173 3000             app at /, its API at /_port/3000
  bunflared 5173 3000 --detach    print the ready line, keep sharing in the background
  bunflared ls [--json]           list live shares
  bunflared down <id> | --all     stop shares
  bunflared agents                teach your coding agents to use bunflared

In a terminal you get the animated dashboard: ? lists its keys. m messages the
visitors, g sends them to a page, R reloads it, Enter on a request shows it and
p replays it. Notes, chat and reactions are saved in bunflared-feedback/.

Otherwise, or with --json, the ready line is one JSON object on stdout:
  {"id":"4242","pid":4242,"tunnel_pid":4243,"url":"https://....trycloudflare.com",
   "routes":{"/":5173,"/_port/3000":3000},"started_at":1790000000}
and a failure is one JSON object on stderr: {"error":"...","code":N}.

cloudflared is fetched from Cloudflare's releases on the first run when it is
not installed.

Exit codes: 0 ok, 1 tunnel closed, 2 bad arguments, 3 cloudflared missing and
not downloadable, 4 a port is not answering, 5 ~/.cloudflared/config.yaml blocks quick tunnels,
6 the tunnel failed to start.

Limits: anyone with the link reaches your app. Quick tunnels allow 200
requests in flight and do not carry Server-Sent Events."#;

#[derive(Parser)]
#[command(
    version,
    about = "Share local ports on a temporary public https link.",
    after_help = AFTER_HELP,
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Ports to share. The first is served at "/", the others under "/_port/<port>".
    #[arg(required = true, value_parser = clap::value_parser!(u16).range(1..))]
    ports: Vec<u16>,

    /// No dashboard: print one JSON line once the link is live.
    #[arg(long)]
    json: bool,

    /// Print the JSON line once live, then return and keep sharing in the background.
    #[arg(long)]
    detach: bool,

    /// No animations. NO_COLOR also turns them off, with the colors.
    #[arg(long)]
    calm: bool,

    /// Leave the pages as they are: no feedback button, no live session.
    #[arg(long)]
    no_widget: bool,

    /// Colors for a light or dark terminal. Auto asks the terminal.
    #[arg(long, value_enum, default_value_t = ThemeChoice::Auto)]
    theme: ThemeChoice,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ThemeChoice {
    Auto,
    Light,
    Dark,
}

#[derive(Subcommand)]
enum Command {
    /// List live shares.
    Ls {
        #[arg(long)]
        json: bool,
    },
    /// Teach the coding agents installed here (Claude Code, Codex, pi, omp, prime-agent) to use bunflared.
    Agents {
        /// Print the note instead, to paste into another agent's rules.
        #[arg(long)]
        print: bool,
        /// Take the note back out.
        #[arg(long, conflicts_with = "print")]
        remove: bool,
    },
    /// Stop a share by id, or all of them.
    Down {
        #[arg(required_unless_present = "all")]
        id: Option<String>,
        #[arg(long)]
        all: bool,
    },
}

fn main() {
    // A panic on the main thread must not leave cloudflared running.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().name() == Some("main") {
            tunnel::kill_orphan();
        }
        default_hook(info);
    }));

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) if err.use_stderr() && machine_output() => {
            let message = err.to_string();
            let summary = message.split("\n\n").next().unwrap_or("bad arguments");
            let summary = summary.split_whitespace().collect::<Vec<_>>().join(" ");
            let json = serde_json::json!({ "error": summary.trim_start_matches("error: "), "code": EXIT_USAGE });
            eprintln!("{json}");
            std::process::exit(EXIT_USAGE);
        }
        Err(err) => err.exit(),
    };
    let code = match cli.command {
        Some(Command::Ls { json }) => state::list(json),
        Some(Command::Down { id, all }) => state::down(id, all),
        Some(Command::Agents { print, remove }) => agents::run(print, remove),
        None if cli.detach => state::detach(&cli.ports, cli.no_widget),
        None => {
            let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
            let interactive = !cli.json && std::io::stdout().is_terminal();
            let theme = interactive.then(|| tui::Theme {
                calm: cli.calm || no_color,
                color: !no_color,
                light: match cli.theme {
                    ThemeChoice::Light => true,
                    ThemeChoice::Dark => false,
                    ThemeChoice::Auto => tui::light_terminal(),
                },
            });
            let feedback = (!cli.no_widget)
                .then(|| std::env::current_dir().ok())
                .flatten()
                .map(|dir| dir.join(widget::FOLDER));
            share(cli.ports, theme, feedback)
        }
    };
    std::process::exit(code);
}

fn machine_output() -> bool {
    std::env::args().any(|arg| arg == "--json" || arg == "--detach")
        || !std::io::stdout().is_terminal()
}

/// Runs the share with the dashboard when a theme is given, JSON otherwise.
fn share(ports: Vec<u16>, theme: Option<tui::Theme>, feedback: Option<std::path::PathBuf>) -> i32 {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let (tx, rx) = mpsc::channel();
    let (stop, stop_rx) = watch::channel(false);
    let hub = std::sync::Arc::new(live::Hub::new(tx.clone(), feedback.clone()));
    let shared = ports.clone();
    let backend_hub = hub.clone();
    let replayer = proxy::Replayer {
        runtime: runtime.handle().clone(),
        tx: tx.clone(),
    };
    let backend = runtime.spawn(async move {
        let result = share::run(&shared, &tx, stop_rx, feedback, backend_hub).await;
        let _ = tx.send(Event::Done(result));
    });

    let code = match theme {
        Some(theme) => tui::run(rx, &stop, &ports, theme, hub, replayer),
        None => print_json(rx),
    };

    let _ = stop.send(true);
    runtime.block_on(async {
        let _ = tokio::time::timeout(SHUTDOWN_TIMEOUT, backend).await;
    });
    code
}

// Writes ignore errors: a detached share outlives the pipe it was started with.
fn print_json(rx: mpsc::Receiver<Event>) -> i32 {
    for event in rx {
        match event {
            Event::Ready { record, .. } => {
                let mut out = std::io::stdout();
                let _ = writeln!(
                    out,
                    "{}",
                    serde_json::to_string(&record).unwrap_or_default()
                );
                let _ = out.flush();
            }
            Event::Done(Ok(())) => return 0,
            Event::Done(Err(failure)) => {
                let json = serde_json::json!({ "error": failure.message, "code": failure.code });
                let _ = writeln!(std::io::stderr(), "{json}");
                return failure.code;
            }
            _ => {}
        }
    }
    0
}
