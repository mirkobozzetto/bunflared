//! The live channel: each page carrying the widget keeps a WebSocket open to
//! bunflared, so the dashboard can talk to visitors and drive the demo.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::protocol::Message;

use crate::event::{Event, PageError, Pointer, Reacted, Said, Tx};
use crate::look::{Looked, Looks};
use crate::notes::Transcript;

mod handshake;
pub use handshake::{absolute, accept, valid_sid};

const MAX_TEXT: usize = 2000;
const ERROR_KINDS: [&str; 3] = ["error", "rejection", "console"];
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
const CLOSING_SOON: &str = "This link closes in a minute.";

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
    /// A screenshot for the agent, of the screen or of one element, after a
    /// reload when asked.
    Look {
        id: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        selector: Option<String>,
        reload: bool,
    },
}

/// What pages send to the dashboard.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Incoming {
    Chat {
        text: String,
        page: String,
    },
    Pointer {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
    React {
        emoji: String,
        #[serde(default)]
        page: String,
    },
    Shot(Looked),
    Error {
        kind: String,
        message: String,
        #[serde(default)]
        source: String,
        #[serde(default)]
        stack: String,
        #[serde(default)]
        page: String,
        #[serde(default)]
        count: u32,
    },
}

struct Socket {
    sid: String,
    out: UnboundedSender<String>,
}

pub struct Hub {
    sockets: Mutex<HashMap<u64, Socket>>,
    next: AtomicU64,
    tx: Tx,
    /// In the feedback folder, when there is one.
    transcript: Option<Transcript>,
    followed: Mutex<Option<String>>,
    looks: Looks,
    /// Set in the last minute of `--for`, so pages that connect then hear it too.
    closing: AtomicBool,
}

impl Hub {
    pub fn new(tx: Tx, folder: Option<PathBuf>) -> Self {
        Self {
            sockets: Mutex::default(),
            next: AtomicU64::new(0),
            tx,
            transcript: folder.map(Transcript::new),
            followed: Mutex::default(),
            looks: Looks::default(),
            closing: AtomicBool::new(false),
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

    /// Tells every page, and every page that connects from now on, that the
    /// link is about to close.
    pub fn closing_soon(&self) {
        self.closing.store(true, Ordering::SeqCst);
        self.say(None, "everyone", CLOSING_SOON);
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

    /// Asks the visitor's latest page for a screenshot and waits for it.
    pub fn look(
        &self,
        sid: &str,
        selector: Option<&str>,
        reload: bool,
        wait: Duration,
    ) -> Result<Looked, String> {
        self.looks.ask(wait, |id| {
            let selector = selector.map(str::to_string);
            let command = Command::Look {
                id,
                selector,
                reload,
            };
            self.send_latest(sid, &command)
        })
    }

    /// The visitors with a page connected live, each once.
    pub fn connected(&self) -> Vec<String> {
        let sockets = self.sockets.lock().unwrap();
        let mut sids: Vec<String> = sockets.values().map(|s| s.sid.clone()).collect();
        sids.sort();
        sids.dedup();
        sids
    }

    /// Sends to the page a visitor opened last, so only one answers.
    fn send_latest(&self, sid: &str, command: &Command) -> bool {
        let Ok(text) = serde_json::to_string(command) else {
            return false;
        };
        let sockets = self.sockets.lock().unwrap();
        sockets
            .iter()
            .filter(|(_, socket)| socket.sid == sid)
            .max_by_key(|(id, _)| **id)
            .is_some_and(|(_, socket)| socket.out.send(text).is_ok())
    }

    fn receive(&self, sid: &str, device: &str, message: &str, reacted: &mut Option<Instant>) {
        match serde_json::from_str(message) {
            Ok(Incoming::React { emoji, page }) => {
                let kind = REACTIONS.iter().position(|(known, _)| *known == emoji);
                let calm = reacted.is_none_or(|at| at.elapsed() >= REACT_EVERY);
                if let (Some(kind), true) = (kind, calm) {
                    *reacted = Some(Instant::now());
                    let page: String = page.chars().take(MAX_TEXT).collect();
                    self.keep(&format!("**{device}** on `{page}` reacted {emoji}"));
                    let device = device.to_string();
                    let _ = self.tx.send(Event::Reacted(Reacted { device, kind }));
                }
            }
            Ok(Incoming::Chat { text, page }) => self.chat(sid, device, text, page),
            Ok(Incoming::Shot(looked)) => self.looks.answer(looked),
            Ok(Incoming::Error {
                kind,
                message,
                source,
                stack,
                page,
                count,
            }) if ERROR_KINDS.contains(&kind.as_str()) => {
                let cut = |text: String| text.chars().take(MAX_TEXT).collect();
                let _ = self.tx.send(Event::PageError(PageError {
                    sid: sid.to_string(),
                    device: device.to_string(),
                    page: cut(page),
                    kind,
                    message: cut(message),
                    source: cut(source),
                    stack: cut(stack),
                    count: count.max(1),
                }));
            }
            Ok(Incoming::Pointer { x, y, w, h }) => {
                let sid = sid.to_string();
                let _ = self.tx.send(Event::Pointer(Pointer { sid, x, y, w, h }));
            }
            Ok(Incoming::Error { .. }) | Err(_) => {}
        }
    }

    fn chat(&self, sid: &str, device: &str, text: String, page: String) {
        let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let text: String = text.chars().take(MAX_TEXT).collect();
        let page: String = page.chars().take(MAX_TEXT).collect();
        if text.is_empty() {
            return;
        }
        self.keep(&format!("**{device}** on `{page}`: {text}"));
        let _ = self.tx.send(Event::Chat(Said {
            sid: sid.to_string(),
            device: device.to_string(),
            page,
            text,
        }));
    }

    /// Appends a line to this share's chat transcript, next to the notes.
    /// Sends a reaction, and keeps it in the session log once a page has it.
    pub fn react(&self, sid: Option<&str>, to: &str, emoji: &str) -> usize {
        let emoji = emoji.to_string();
        let reached = self.send(
            sid,
            &Command::React {
                emoji: emoji.clone(),
            },
        );
        if reached > 0 {
            self.keep(&format!("**you → {to}** reacted {emoji}"));
        }
        reached
    }

    /// Points the session log at a note saved next to it.
    pub fn noted(&self, device: &str, page: &str, note: &str) {
        self.keep(&format!(
            "**{device}** on `{page}` left a note: [{note}]({note})"
        ));
    }

    fn keep(&self, line: &str) {
        if let Some(transcript) = &self.transcript {
            transcript.keep(line);
        }
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
        if self.closing.load(Ordering::SeqCst)
            && let Ok(text) = serde_json::to_string(&Command::Chat {
                text: CLOSING_SOON.into(),
            })
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
