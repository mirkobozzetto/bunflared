mod clipboard;
mod proxy;
mod share;
mod state;
mod tunnel;

use std::io::Write;
use std::sync::mpsc;
use std::time::Duration;

use clap::Parser;
use tokio::sync::watch;

use share::Event;

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Parser)]
#[command(version, about = "Share local ports on a temporary public https link.")]
struct Cli {
    /// Ports to share. The first is served at "/", the others under "/_port/<port>".
    #[arg(required = true, value_parser = clap::value_parser!(u16).range(1..))]
    ports: Vec<u16>,
}

fn main() {
    let cli = Cli::parse();
    std::process::exit(share(cli.ports));
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
