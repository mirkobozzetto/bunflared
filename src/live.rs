//! The live channel: each page carrying the widget keeps a WebSocket open to
//! bunflared, so the dashboard can talk to visitors and drive the demo.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use hyper::body::Incoming;
use hyper::header::{self, HeaderMap, HeaderValue};
use hyper::upgrade::Upgraded;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::protocol::{Message, Role, WebSocketConfig};

use crate::proxy::{Body, full};

const MAX_MESSAGE: usize = 16 * 1024;
const MAX_SID: usize = 32;
// Proxies drop quiet WebSockets; a ping now and then keeps this one open.
const KEEPALIVE: Duration = Duration::from_secs(30);

/// What the dashboard sends to pages.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Command {
    Go { path: String },
    Reload,
}

struct Socket {
    sid: String,
    out: UnboundedSender<String>,
}

#[derive(Default)]
pub struct Hub {
    sockets: Mutex<HashMap<u64, Socket>>,
    next: AtomicU64,
}

impl Hub {
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

    async fn serve(self: Arc<Self>, sid: String, socket: WebSocketStream<TokioIo<Upgraded>>) {
        let (mut sink, mut stream) = socket.split();
        let (out, mut outbox) = unbounded_channel();
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.sockets.lock().unwrap().insert(id, Socket { sid, out });
        let mut keepalive = tokio::time::interval(KEEPALIVE);
        loop {
            let sent = tokio::select! {
                Some(text) = outbox.recv() => sink.send(Message::text(text)).await,
                _ = keepalive.tick() => sink.send(Message::Ping(Bytes::new())).await,
                // A Close is answered by the next read, which then ends the stream.
                incoming = stream.next() => match incoming {
                    Some(Err(_)) | None => break,
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
pub fn accept(mut request: Request<Incoming>, hub: Arc<Hub>) -> Response<Body> {
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
        hub.serve(sid, socket).await;
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
