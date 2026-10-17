//! How a page joins the live channel: the WebSocket handshake, and the ids
//! and paths it may carry.

use std::sync::Arc;

use hyper::header::{self, HeaderMap, HeaderValue};
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::protocol::{Role, WebSocketConfig};

use super::Hub;
use crate::http::{Body, status};

const MAX_MESSAGE: usize = 16 * 1024;
const MAX_SID: usize = 32;

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
        .filter(|sid| valid_sid(sid))
        .map(str::to_string)
}

/// A page path as the widget's go expects it, from what was typed.
pub fn absolute(path: &str) -> String {
    let path = path.trim();
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

/// A visitor id as the widget makes them: short, letters and digits.
pub fn valid_sid(sid: &str) -> bool {
    !sid.is_empty() && sid.len() <= MAX_SID && sid.chars().all(|c| c.is_ascii_alphanumeric())
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
