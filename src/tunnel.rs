use std::path::Path;
use std::process::Stdio;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use regex::Regex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UdpSocket;
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};

use crate::share::{EXIT_TUNNEL_FAILED, Event, Failure, Tx};

// The hyphen excludes api.trycloudflare.com, which cloudflared names in its errors.
static TUNNEL_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https://[a-z0-9]+(?:-[a-z0-9]+)+\.trycloudflare\.com").unwrap());

const URL_TIMEOUT: Duration = Duration::from_secs(45);
const DNS_SERVER: &str = "1.1.1.1:53";
const DNS_ATTEMPTS: u32 = 90;
const DNS_TIMEOUT: Duration = Duration::from_secs(2);
const LOG_TAIL: usize = 12;

/// The cloudflared pid, so a panic hook can kill it without reaching the runtime.
pub static PID: AtomicU32 = AtomicU32::new(0);

pub struct Tunnel {
    pub child: Child,
    pub url: String,
}

// The child is spawned with kill_on_drop: dropping the tunnel kills cloudflared.
// SIGKILL on purpose, on SIGTERM cloudflared lingers through a 30 s grace period.
impl Drop for Tunnel {
    fn drop(&mut self) {
        PID.store(0, Ordering::Relaxed);
    }
}

pub fn kill_orphan() {
    let pid = PID.swap(0, Ordering::Relaxed);
    if pid != 0 {
        unsafe { libc::kill(pid as i32, libc::SIGKILL) };
    }
}

pub async fn open(cloudflared: &Path, proxy_port: u16, tx: &Tx) -> Result<Tunnel, Failure> {
    let mut child = Command::new(cloudflared)
        .args(["tunnel", "--no-autoupdate", "--url"])
        .arg(format!("http://127.0.0.1:{proxy_port}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|err| {
            Failure::new(
                EXIT_TUNNEL_FAILED,
                format!("cannot start cloudflared: {err}"),
            )
        })?;
    PID.store(child.id().unwrap_or(0), Ordering::Relaxed);

    let mut lines = BufReader::new(child.stderr.take().expect("piped stderr")).lines();
    let mut tail = Vec::new();
    let found = timeout(URL_TIMEOUT, async {
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(url) = TUNNEL_URL.find(&line) {
                return Some(url.as_str().to_string());
            }
            tail.push(line);
        }
        None
    })
    .await;
    let Ok(Some(url)) = found else {
        let _ = child.start_kill();
        let start = tail.len().saturating_sub(LOG_TAIL);
        let log = tail[start..].join("\n");
        return Err(Failure::new(
            EXIT_TUNNEL_FAILED,
            format!("cloudflared gave no address.\n{log}")
                .trim_end()
                .to_string(),
        ));
    };
    let _ = tx.send(Event::TunnelUrl(url.clone()));

    // Keep draining stderr so cloudflared never blocks on a full pipe.
    let tx = tx.clone();
    tokio::spawn(async move {
        let mut registered = false;
        while let Ok(Some(line)) = lines.next_line().await {
            if !registered && line.contains("Registered tunnel connection") {
                registered = true;
                let _ = tx.send(Event::TunnelRegistered);
            }
        }
    });
    Ok(Tunnel { child, url })
}

/// A new tunnel name takes a few seconds to exist, and a lookup made before that
/// is cached as "not found" for minutes by the local resolver. Ask 1.1.1.1
/// directly and report once the name resolves.
pub async fn wait_dns(host: &str, tx: &Tx) -> bool {
    for attempt in 1..=DNS_ATTEMPTS {
        let _ = tx.send(Event::DnsAttempt(attempt));
        if resolves(host).await {
            return true;
        }
        sleep(Duration::from_secs(1)).await;
    }
    false
}

async fn resolves(host: &str) -> bool {
    let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await else {
        return false;
    };
    let query = dns_query(host);
    if socket.send_to(&query, DNS_SERVER).await.is_err() {
        return false;
    }
    let mut answer = [0u8; 512];
    match timeout(DNS_TIMEOUT, socket.recv(&mut answer)).await {
        Ok(Ok(len)) if len >= 12 => {
            let same_id = answer[..2] == query[..2];
            let no_error = answer[3] & 0x0f == 0;
            let answers = u16::from_be_bytes([answer[6], answer[7]]);
            same_id && no_error && answers > 0
        }
        _ => false,
    }
}

fn dns_query(host: &str) -> Vec<u8> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let id = (nanos as u16) ^ (std::process::id() as u16);
    let mut query = id.to_be_bytes().to_vec();
    // Recursion desired, one question.
    query.extend_from_slice(&[0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0]);
    for label in host.split('.') {
        query.push(label.len() as u8);
        query.extend_from_slice(label.as_bytes());
    }
    // End of name, type A, class IN.
    query.extend_from_slice(&[0, 0, 1, 0, 1]);
    query
}
