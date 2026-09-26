//! The live channel: each page carrying the widget keeps a WebSocket open to
//! bunflared, so the dashboard can talk to visitors and drive the demo.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use hyper::header::{self, HeaderMap, HeaderValue};
use hyper::upgrade::Upgraded;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::protocol::{Message, Role, WebSocketConfig};

use crate::proxy::{Body, full};
use crate::share::{Event, Tx};
use crate::widget;

const MAX_MESSAGE: usize = 16 * 1024;
const MAX_TEXT: usize = 2000;
const MAX_SID: usize = 32;
// A visitor holding a reaction button down must not flood the dashboard.
const REACT_EVERY: Duration = Duration::from_millis(250);
pub const REACTIONS: [(&str, &str); 4] = [
    ("👍", "thumbs up"),
    ("🔥", "on fire"),
    ("😍", "love it"),
    ("😕", "not sure"),
];
// Proxies drop quiet WebSockets; a ping now and then keeps this one open.
const KEEPALIVE: Duration = Duration::from_secs(30);

/// What the dashboard sends to pages.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Command {
    Go {
        path: String,
    },
    Reload,
    Chat {
        text: String,
    },
    /// The dashboard watches this page's pointer, or stops.
    Follow {
        on: bool,
    },
    React {
        emoji: String,
    },
}

/// What pages send to the dashboard.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Incoming {
    Chat { text: String, page: String },
    Pointer { x: f32, y: f32, w: f32, h: f32 },
    React { emoji: String },
}

/// A reaction from a visitor, as an index in `REACTIONS`.
#[derive(Debug, Clone)]
pub struct Reacted {
    pub device: String,
    pub kind: usize,
}

/// Where a followed visitor's pointer is, in their viewport.
#[derive(Debug, Clone)]
pub struct Pointer {
    pub sid: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// A chat message from a visitor.
#[derive(Debug, Clone)]
pub struct Said {
    pub device: String,
    pub page: String,
    pub text: String,
}

struct Socket {
    sid: String,
    out: UnboundedSender<String>,
}

pub struct Hub {
    sockets: Mutex<HashMap<u64, Socket>>,
    next: AtomicU64,
    tx: Tx,
    /// The feedback folder, where the chat transcript goes too.
    folder: Option<PathBuf>,
    transcript: Mutex<Option<PathBuf>>,
    followed: Mutex<Option<String>>,
}

impl Hub {
    pub fn new(tx: Tx, folder: Option<PathBuf>) -> Self {
        Self {
            sockets: Mutex::default(),
            next: AtomicU64::new(0),
            tx,
            folder,
            transcript: Mutex::default(),
            followed: Mutex::default(),
        }
    }

    /// Asks one visitor's pages for their pointer, and the previous one's to stop.
    pub fn follow(&self, sid: Option<&str>) {
        let mut followed = self.followed.lock().unwrap();
        if followed.as_deref() == sid {
            return;
        }
        if let Some(old) = followed.take() {
            self.send(Some(&old), &Command::Follow { on: false });
        }
        if let Some(sid) = sid {
            self.send(Some(sid), &Command::Follow { on: true });
            *followed = Some(sid.to_string());
        }
    }

    /// Sends a chat message, and keeps it in the transcript once a page has it.
    pub fn say(&self, sid: Option<&str>, to: &str, text: &str) -> usize {
        let text = text.to_string();
        let reached = self.send(sid, &Command::Chat { text: text.clone() });
        if reached > 0 {
            self.keep(&format!("**you → {to}**: {text}"));
        }
        reached
    }

    /// Sends to the pages of one visitor, or of everyone. Returns how many
    /// pages it reached.
    pub fn send(&self, sid: Option<&str>, command: &Command) -> usize {
        let Ok(text) = serde_json::to_string(command) else {
            return 0;
        };
        let sockets = self.sockets.lock().unwrap();
        sockets
            .values()
            .filter(|socket| sid.is_none_or(|sid| socket.sid == sid))
            .filter(|socket| socket.out.send(text.clone()).is_ok())
            .count()
    }

    fn receive(&self, sid: &str, device: &str, message: &str, reacted: &mut Option<Instant>) {
        match serde_json::from_str(message) {
            Ok(Incoming::React { emoji }) => {
                let kind = REACTIONS.iter().position(|(known, _)| *known == emoji);
                let calm = reacted.is_none_or(|at| at.elapsed() >= REACT_EVERY);
                if let (Some(kind), true) = (kind, calm) {
                    *reacted = Some(Instant::now());
                    let device = device.to_string();
                    let _ = self.tx.send(Event::Reacted(Reacted { device, kind }));
                }
            }
            Ok(Incoming::Chat { text, page }) => self.chat(device, text, page),
            Ok(Incoming::Pointer { x, y, w, h }) => {
                let sid = sid.to_string();
                let _ = self.tx.send(Event::Pointer(Pointer { sid, x, y, w, h }));
            }
            Err(_) => {}
        }
    }

    fn chat(&self, device: &str, text: String, page: String) {
        let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let text: String = text.chars().take(MAX_TEXT).collect();
        let page: String = page.chars().take(MAX_TEXT).collect();
        if text.is_empty() {
            return;
        }
        self.keep(&format!("**{device}** on `{page}`: {text}"));
        let _ = self.tx.send(Event::Chat(Said {
            device: device.to_string(),
            page,
            text,
        }));
    }

    /// Appends a line to this share's chat transcript, next to the notes.
    fn keep(&self, line: &str) {
        let Some(folder) = &self.folder else {
            return;
        };
        let mut transcript = self.transcript.lock().unwrap();
        let path =
            transcript.get_or_insert_with(|| folder.join(format!("chat_{}.md", widget::stamp())));
        let fresh = !path.exists();
        if widget::folder_ready(folder).is_err() {
            return;
        }
        let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
            return;
        };
        let now = chrono::Local::now();
        if fresh {
            let _ = writeln!(file, "# Chat, {}\n", now.format("%Y-%m-%d %H:%M"));
        }
        let _ = writeln!(file, "- {} {line}", now.format("%H:%M:%S"));
    }

    async fn serve(
        self: Arc<Self>,
        sid: String,
        device: String,
        socket: WebSocketStream<TokioIo<Upgraded>>,
    ) {
        let (mut sink, mut stream) = socket.split();
        let (out, mut outbox) = unbounded_channel();
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        // A followed visitor's new page is followed too.
        if self.followed.lock().unwrap().as_deref() == Some(sid.as_str())
            && let Ok(text) = serde_json::to_string(&Command::Follow { on: true })
        {
            let _ = out.send(text);
        }
        let socket = Socket {
            sid: sid.clone(),
            out,
        };
        self.sockets.lock().unwrap().insert(id, socket);
        let mut keepalive = tokio::time::interval(KEEPALIVE);
        let mut reacted = None;
        loop {
            let sent = tokio::select! {
                Some(text) = outbox.recv() => sink.send(Message::text(text)).await,
                _ = keepalive.tick() => sink.send(Message::Ping(Bytes::new())).await,
                // A Close is answered by the next read, which then ends the stream.
                incoming = stream.next() => match incoming {
                    Some(Err(_)) | None => break,
                    Some(Ok(Message::Text(text))) => {
                        self.receive(&sid, &device, &text, &mut reacted);
                        Ok(())
                    }
                    Some(Ok(_)) => Ok(()),
                },
            };
            if sent.is_err() {
                break;
            }
        }
        self.sockets.lock().unwrap().remove(&id);
    }
}

/// Answers the WebSocket handshake of `/_bunflared/live?sid=...` and hands
/// the connection to the hub.
pub fn accept(
    mut request: Request<hyper::body::Incoming>,
    hub: Arc<Hub>,
    device: String,
) -> Response<Body> {
    let headers = request.headers();
    let upgrade = headers
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("websocket"));
    let key = headers.get(header::SEC_WEBSOCKET_KEY).cloned();
    let sid = request.uri().query().and_then(sid);
    let (true, Some(key), Some(sid), true) = (upgrade, key, sid, same_origin(headers)) else {
        return status(StatusCode::BAD_REQUEST);
    };
    let accept = derive_accept_key(key.as_bytes());
    let upgraded = hyper::upgrade::on(&mut request);
    tokio::spawn(async move {
        let Ok(upgraded) = upgraded.await else {
            return;
        };
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_MESSAGE))
            .max_frame_size(Some(MAX_MESSAGE));
        let socket =
            WebSocketStream::from_raw_socket(TokioIo::new(upgraded), Role::Server, Some(config))
                .await;
        hub.serve(sid, device, socket).await;
    });

    let mut response = status(StatusCode::SWITCHING_PROTOCOLS);
    let headers = response.headers_mut();
    headers.insert(header::UPGRADE, HeaderValue::from_static("websocket"));
    headers.insert(header::CONNECTION, HeaderValue::from_static("Upgrade"));
    if let Ok(value) = HeaderValue::from_str(&accept) {
        headers.insert(header::SEC_WEBSOCKET_ACCEPT, value);
    }
    response
}

fn sid(query: &str) -> Option<String> {
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("sid="))
        .filter(|sid| {
            !sid.is_empty()
                && sid.len() <= MAX_SID
                && sid.chars().all(|c| c.is_ascii_alphanumeric())
        })
        .map(str::to_string)
}

/// Another site's page must not listen in on the messages meant for visitors.
fn same_origin(headers: &HeaderMap) -> bool {
    let value = |name| {
        headers
            .get(name)
            .and_then(|v: &HeaderValue| v.to_str().ok())
    };
    match (value(header::ORIGIN), value(header::HOST)) {
        (Some(origin), Some(host)) => origin
            .split_once("://")
            .is_some_and(|(_, rest)| rest == host),
        (None, _) => true,
        _ => false,
    }
}

fn status(code: StatusCode) -> Response<Body> {
    let mut response = Response::new(full(Bytes::new()));
    *response.status_mut() = code;
    response
}
