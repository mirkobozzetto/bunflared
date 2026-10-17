//! What the share tells its watchers, the dashboard and the MCP server, and
//! the records those messages carry.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use hyper::header::{self, HeaderMap};

use crate::notes::Feedback;
use crate::state::Record;
use crate::visitors::Presence;

pub const EXIT_TUNNEL_CLOSED: i32 = 1;
pub const EXIT_NO_CLOUDFLARED: i32 = 3;
pub const EXIT_PORT_DOWN: i32 = 4;
pub const EXIT_CONFIG_YAML: i32 = 5;
pub const EXIT_TUNNEL_FAILED: i32 = 6;
pub const EXIT_HANGUP: i32 = 129;

/// How much of each body the inspector keeps; a request over it cannot be replayed.
pub const CAPTURE: usize = 32 * 1024;
/// Content types the inspector shows as text.
const READABLE: [&str; 8] = [
    "text/",
    "json",
    "javascript",
    "xml",
    "html",
    "css",
    "form-urlencoded",
    "graphql",
];

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
    pub exchange: Arc<Exchange>,
}

impl Hit {
    /// Why it cannot be sent again as it was, if it cannot.
    pub fn unreplayable(&self) -> Option<String> {
        if self.upgrade {
            Some("A WebSocket cannot be sent again.".into())
        } else if !self.exchange.request_body.lock().unwrap().complete() {
            Some(format!(
                "Its body is over {} KiB: only the start was kept.",
                CAPTURE / 1024
            ))
        } else {
            None
        }
    }
}

/// The start of a body as it went through, and its full size.
#[derive(Debug, Default)]
pub struct Capture {
    pub bytes: Vec<u8>,
    pub total: usize,
}

impl Capture {
    pub fn complete(&self) -> bool {
        self.bytes.len() == self.total
    }

    /// Whether the kept start can be shown as text, going by its headers.
    pub fn readable(&self, headers: &HeaderMap) -> bool {
        let kind = headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        !headers.contains_key(header::CONTENT_ENCODING)
            && if kind.is_empty() {
                std::str::from_utf8(&self.bytes).is_ok()
            } else {
                READABLE.iter().any(|t| kind.contains(t))
            }
    }
}

/// What the inspector shows of a request and its answer.
#[derive(Debug)]
pub struct Exchange {
    pub request_headers: HeaderMap,
    pub response_headers: HeaderMap,
    pub request_body: Arc<Mutex<Capture>>,
    pub response_body: Arc<Mutex<Capture>>,
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
    pub sid: String,
    pub device: String,
    pub page: String,
    pub text: String,
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
        record: Record,
        resolved: bool,
        copied: bool,
    },
    Request(Hit),
    Presence(Presence),
    Feedback(Feedback),
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
