mod clipboard;
mod proxy;
mod share;
mod state;
mod tunnel;

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

In a terminal you get the animated dashboard. Otherwise, or with --json, the
ready line is one JSON object on stdout:
  {"id":"4242","pid":4242,"tunnel_pid":4243,"url":"https://....trycloudflare.com",
   "routes":{"/":5173,"/_port/3000":3000},"started_at":1790000000}
and a failure is one JSON object on stderr: {"error":"...","code":N}.

Exit codes: 0 ok, 1 tunnel closed, 2 bad arguments, 3 cloudflared missing,
4 a port is not answering, 5 ~/.cloudflared/config.yaml blocks quick tunnels,
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
}

#[derive(Subcommand)]
enum Command {
    /// List live shares.
    Ls {
        #[arg(long)]
        json: bool,
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
        None if cli.detach => state::detach(&cli.ports),
        None => share(cli.ports),
    };
    std::process::exit(code);
}

fn machine_output() -> bool {
    std::env::args().any(|arg| arg == "--json" || arg == "--detach")
        || !std::io::stdout().is_terminal()
}

fn share(ports: Vec<u16>) -> i32 {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let (tx, rx) = mpsc::channel();
    let (stop, stop_rx) = watch::channel(false);
    let backend = runtime.spawn(async move {
        let result = share::run(&ports, &tx, stop_rx).await;
        let _ = tx.send(Event::Done(result));
    });

    let code = print_json(rx);

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
