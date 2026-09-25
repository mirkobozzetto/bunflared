use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::net::TcpStream;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::watch;
use tokio::time::{sleep, timeout};

use crate::{clipboard, proxy, state, tunnel};

pub const EXIT_TUNNEL_CLOSED: i32 = 1;
pub const EXIT_NO_CLOUDFLARED: i32 = 3;
pub const EXIT_PORT_DOWN: i32 = 4;
pub const EXIT_CONFIG_YAML: i32 = 5;
pub const EXIT_TUNNEL_FAILED: i32 = 6;

const PORT_TIMEOUT: Duration = Duration::from_secs(3);
const HEALTH_EVERY: Duration = Duration::from_secs(3);

pub type Tx = Sender<Event>;

#[derive(Debug, Clone)]
pub struct Failure {
    pub code: i32,
    pub message: String,
}

impl Failure {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Hit {
    pub method: String,
    pub path: String,
    pub status: u16,
    pub ms: u32,
    pub port: u16,
    pub visitor: String,
    pub upgrade: bool,
}

#[derive(Debug)]
pub enum Event {
    PortChecked {
        port: u16,
        ok: bool,
    },
    TunnelStarting,
    TunnelUrl(String),
    TunnelRegistered,
    DnsAttempt(u32),
    Ready {
        record: state::Record,
        resolved: bool,
        copied: bool,
    },
    Request(Hit),
    PortHealth {
        port: u16,
        ok: bool,
    },
    Done(Result<(), Failure>),
}

/// Shares `ports` until `stop` flips, SIGINT/SIGTERM arrives, or the tunnel dies.
pub async fn run(ports: &[u16], tx: &Tx, mut stop: watch::Receiver<bool>) -> Result<(), Failure> {
    let cloudflared = which("cloudflared").ok_or_else(|| {
        Failure::new(
            EXIT_NO_CLOUDFLARED,
            "cloudflared is missing: brew install cloudflared",
        )
    })?;
    if let Some(config) = quick_tunnel_blocker() {
        return Err(Failure::new(
            EXIT_CONFIG_YAML,
            format!(
                "{} exists, and quick tunnels refuse to start while it does. Rename it while sharing.",
                config.display()
            ),
        ));
    }
    for &port in ports {
        let ok = answers(port).await;
        let _ = tx.send(Event::PortChecked { port, ok });
        if !ok {
            return Err(Failure::new(
                EXIT_PORT_DOWN,
                format!("Nothing answers on localhost:{port}. Start the app first."),
            ));
        }
    }

    let proxy_port = proxy::start(ports, tx.clone()).await.map_err(|err| {
        Failure::new(EXIT_TUNNEL_FAILED, format!("cannot start the proxy: {err}"))
    })?;
    let _ = tx.send(Event::TunnelStarting);
    let mut tunnel = tunnel::open(&cloudflared, proxy_port, tx).await?;
    let host = tunnel.url.trim_start_matches("https://").to_string();

    let mut interrupt = signal(SignalKind::interrupt()).expect("SIGINT handler");
    let mut terminate = signal(SignalKind::terminate()).expect("SIGTERM handler");
    let closed = || Failure::new(EXIT_TUNNEL_CLOSED, "The tunnel closed.");

    let resolved = tokio::select! {
        _ = stop.changed() => return tunnel.close().await,
        _ = interrupt.recv() => return tunnel.close().await,
        _ = terminate.recv() => return tunnel.close().await,
        _ = tunnel.child.wait() => return Err(closed()),
        resolved = tunnel::wait_dns(&host, tx) => resolved,
    };

    let record = state::Record {
        id: std::process::id().to_string(),
        pid: std::process::id(),
        tunnel_pid: tunnel.child.id().unwrap_or(0),
        url: tunnel.url.clone(),
        routes: proxy::routes(ports),
        started_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    };
    let _saved = state::Saved::write(&record);
    let copied = clipboard::copy(&record.url);
    let _ = tx.send(Event::Ready {
        record,
        resolved,
        copied,
    });
    tokio::spawn(watch_health(ports.to_vec(), tx.clone()));

    tokio::select! {
        _ = stop.changed() => tunnel.close().await,
        _ = interrupt.recv() => tunnel.close().await,
        _ = terminate.recv() => tunnel.close().await,
        _ = tunnel.child.wait() => Err(closed()),
    }
}

async fn answers(port: u16) -> bool {
    matches!(
        timeout(PORT_TIMEOUT, TcpStream::connect(("localhost", port))).await,
        Ok(Ok(_))
    )
}

async fn watch_health(ports: Vec<u16>, tx: Tx) {
    let mut last = vec![true; ports.len()];
    loop {
        sleep(HEALTH_EVERY).await;
        for (i, &port) in ports.iter().enumerate() {
            let ok = answers(port).await;
            if ok != last[i] {
                last[i] = ok;
                if tx.send(Event::PortHealth { port, ok }).is_err() {
                    return;
                }
            }
        }
    }
}

fn which(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
}

fn quick_tunnel_blocker() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("HOME")?).join(".cloudflared");
    ["config.yaml", "config.yml"]
        .into_iter()
        .map(|name| dir.join(name))
        .find(|path| path.exists())
}
