use std::path::PathBuf;
use std::sync::Arc;

use std::sync::mpsc::Sender;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio::time::{sleep, timeout};

use crate::live::{Hub, Pointer, Reacted, Said};
use crate::{clipboard, cloudflared, os, proxy, state, tunnel, widget};

pub const EXIT_TUNNEL_CLOSED: i32 = 1;
pub const EXIT_NO_CLOUDFLARED: i32 = 3;
pub const EXIT_PORT_DOWN: i32 = 4;
pub const EXIT_CONFIG_YAML: i32 = 5;
pub const EXIT_TUNNEL_FAILED: i32 = 6;
pub const EXIT_HANGUP: i32 = 129;

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
    pub exchange: Arc<proxy::Exchange>,
}

impl Hit {
    /// Why it cannot be sent again as it was, if it cannot.
    pub fn unreplayable(&self) -> Option<String> {
        if self.upgrade {
            Some("A WebSocket cannot be sent again.".into())
        } else if !self.exchange.request_body.lock().unwrap().complete() {
            Some(format!(
                "Its body is over {} KiB: only the start was kept.",
                proxy::CAPTURE / 1024
            ))
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub enum Event {
    PortChecked {
        port: u16,
        ok: bool,
    },
    FetchingCloudflared,
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
    Presence(widget::Presence),
    Feedback(widget::Feedback),
    Chat(Said),
    Pointer(Pointer),
    Reacted(Reacted),
    Replayed {
        n: u32,
        status: Result<u16, String>,
        ms: u32,
    },
    PortHealth {
        port: u16,
        ok: bool,
    },
    Done(Result<(), Failure>),
}

/// What to share, and how.
pub struct Spec {
    pub ports: Vec<u16>,
    /// Served on this computer only, without a tunnel.
    pub local: bool,
    /// The record's id, the process id for a share of its own.
    pub id: String,
    /// Put the address on the clipboard once it works.
    pub copy: bool,
}

/// Shares until `stop` flips, a signal arrives, or the tunnel dies.
/// Dropping the share on the way out kills cloudflared and removes its record.
pub async fn run(
    spec: &Spec,
    tx: &Tx,
    mut stop: watch::Receiver<bool>,
    feedback: Option<PathBuf>,
    hub: Arc<Hub>,
) -> Result<(), Failure> {
    let hung_up = tokio::select! {
        result = serve(spec, tx, feedback, hub) => return result,
        _ = stop.changed() => false,
        hung_up = os::quit() => hung_up,
    };
    // The share is dropped by now. With the terminal gone, the dashboard's
    // input loop spins on a dead tty and would never see Done: leave here.
    if hung_up {
        std::process::exit(EXIT_HANGUP);
    }
    Ok(())
}

/// Runs the share until its tunnel dies; a local share runs until dropped.
pub async fn serve(
    spec: &Spec,
    tx: &Tx,
    feedback: Option<PathBuf>,
    hub: Arc<Hub>,
) -> Result<(), Failure> {
    let ports = &spec.ports[..];
    if !spec.local
        && let Some(config) = quick_tunnel_blocker()
    {
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

    let cloudflared = match (spec.local, cloudflared::find()) {
        (true, _) => None,
        (false, Some(path)) => Some(path),
        (false, None) => {
            let _ = tx.send(Event::FetchingCloudflared);
            Some(cloudflared::fetch().await.map_err(|err| {
                Failure::new(
                    EXIT_NO_CLOUDFLARED,
                    format!(
                        "cloudflared is missing and could not be downloaded ({err}). Install it: {}",
                        cloudflared::install_hint()
                    ),
                )
            })?)
        }
    };

    let proxy_port = proxy::start(ports, tx.clone(), feedback, hub)
        .await
        .map_err(|err| {
            Failure::new(EXIT_TUNNEL_FAILED, format!("cannot start the proxy: {err}"))
        })?;
    let closed = || Failure::new(EXIT_TUNNEL_CLOSED, "The tunnel closed.");
    let (url, mut tunnel, resolved) = match cloudflared {
        None => (format!("http://127.0.0.1:{proxy_port}"), None, true),
        Some(cloudflared) => {
            let _ = tx.send(Event::TunnelStarting);
            let mut tunnel = tunnel::open(&cloudflared, proxy_port, tx).await?;
            let host = tunnel.url.trim_start_matches("https://").to_string();
            let resolved = tokio::select! {
                _ = tunnel.child.wait() => return Err(closed()),
                resolved = tunnel::wait_dns(&host, tx) => resolved,
            };
            (tunnel.url.clone(), Some(tunnel), resolved)
        }
    };

    let record = state::Record {
        id: spec.id.clone(),
        pid: std::process::id(),
        tunnel_pid: tunnel.as_ref().and_then(|t| t.child.id()).unwrap_or(0),
        url,
        routes: proxy::routes(ports),
        started_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    };
    let _saved = state::Saved::write(&record);
    let copied = spec.copy && clipboard::copy(&record.url);
    let _ = tx.send(Event::Ready {
        record,
        resolved,
        copied,
    });
    tokio::spawn(watch_health(ports.to_vec(), tx.clone()));

    match &mut tunnel {
        Some(tunnel) => {
            let _ = tunnel.child.wait().await;
            Err(closed())
        }
        None => std::future::pending().await,
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

fn quick_tunnel_blocker() -> Option<PathBuf> {
    let dir = os::home()?.join(".cloudflared");
    ["config.yaml", "config.yml"]
        .into_iter()
        .map(|name| dir.join(name))
        .find(|path| path.exists())
}
