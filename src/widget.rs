//! The companion script injected into shared pages: live presence for the
//! dashboard, and a feedback tab whose notes and screenshots are saved in
//! the folder bunflared runs from.

use std::path::Path;
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{BodyExt, Limited};
use hyper::body::Incoming;
use hyper::header::{self, HeaderValue};
use hyper::{Method, Request, Response, StatusCode};
use serde::Deserialize;

use crate::event::{Event, Tx};
use crate::http::{Body, full, json, status};
use crate::live::{self, Hub};
use crate::notes::{Feedback, Note, save_note, save_shot};
use crate::visitors::{Presence, device};

pub const PREFIX: &str = "/_bunflared/";
// The page loads one script: its parts share one function scope, in this
// order, and the stylesheet becomes a string inside it.
const SCRIPT: &str = concat!(
    "(() => {\nconst STYLE = String.raw`",
    include_str!("widget/style.css"),
    "`;\n",
    include_str!("widget/presence.js"),
    include_str!("widget/markup.js"),
    include_str!("widget/frame.js"),
    include_str!("widget/capture.js"),
    include_str!("widget/tab.js"),
    include_str!("widget/drag.js"),
    include_str!("widget/panel.js"),
    include_str!("widget/source.js"),
    include_str!("widget/points.js"),
    include_str!("widget/flags.js"),
    include_str!("widget/pointing.js"),
    include_str!("widget/reorder.js"),
    include_str!("widget/tweak.js"),
    include_str!("widget/edit.js"),
    include_str!("widget/note.js"),
    include_str!("widget/chat.js"),
    include_str!("widget/reactions.js"),
    include_str!("widget/live.js"),
    include_str!("widget/boot.js"),
    "})();\n",
);
const TAG: &str = r#"<script src="/_bunflared/widget.js" defer></script>"#;
const MAX_PING: usize = 4 * 1024;
const MAX_NOTE: usize = 64 * 1024;
const MAX_SHOT: usize = 12 * 1024 * 1024;
#[derive(Deserialize)]
struct Ping {
    sid: String,
    page: String,
    visible: bool,
    idle: u32,
    clicks: u32,
    gone: bool,
}

/// Adds the script tag before `</body>`, or at the end when there is none.
pub fn inject(html: &str) -> String {
    // ASCII lowercasing keeps byte offsets, so they index the original.
    let lower = html.to_ascii_lowercase();
    match lower.rfind("</body>").or_else(|| lower.rfind("</html>")) {
        Some(at) => format!("{}{TAG}{}", &html[..at], &html[at..]),
        None => format!("{html}{TAG}"),
    }
}

pub async fn handle(
    request: Request<Incoming>,
    folder: &Path,
    tx: &Tx,
    hub: &Arc<Hub>,
) -> Response<Body> {
    let value = |name: &str| {
        request
            .headers()
            .get(name)
            .and_then(|v: &HeaderValue| v.to_str().ok())
            .unwrap_or("")
    };
    let device = device(value("user-agent"), value("sec-ch-ua"));
    let route = request.uri().path().trim_start_matches(PREFIX).to_string();
    let method = request.method().clone();
    match (method, route.as_str()) {
        (Method::GET, "widget.js") => script(),
        (Method::GET, "live") => live::accept(request, hub.clone(), device),
        (Method::POST, "ping") => match read(request, MAX_PING).await {
            Some(body) => {
                if let Ok(ping) = serde_json::from_slice::<Ping>(&body)
                    && live::valid_sid(&ping.sid)
                {
                    let _ = tx.send(Event::Presence(Presence {
                        sid: ping.sid,
                        device,
                        page: ping.page,
                        visible: ping.visible,
                        idle: ping.idle,
                        clicks: ping.clicks,
                        gone: ping.gone,
                    }));
                }
                status(StatusCode::NO_CONTENT)
            }
            None => status(StatusCode::PAYLOAD_TOO_LARGE),
        },
        (Method::POST, "shot") => match read(request, MAX_SHOT).await {
            Some(image) => match save_shot(folder, &image) {
                Ok(name) => json(serde_json::json!({ "shot": name })),
                Err(_) => status(StatusCode::UNPROCESSABLE_ENTITY),
            },
            None => status(StatusCode::PAYLOAD_TOO_LARGE),
        },
        (Method::POST, "feedback") => {
            let note = read(request, MAX_NOTE)
                .await
                .and_then(|body| serde_json::from_slice::<Note>(&body).ok())
                .filter(Note::written);
            let Some(note) = note else {
                return status(StatusCode::BAD_REQUEST);
            };
            match save_note(folder, &note, &device) {
                Ok((name, shot)) => {
                    hub.noted(&device, &note.page, &name);
                    let _ = tx.send(Event::Feedback(Feedback {
                        sid: note.sid,
                        device,
                        page: note.page,
                        message: note.message.trim().to_string(),
                        file: folder.join(name),
                        shot: shot.map(|shot| folder.join(shot)),
                        points: note.points,
                    }));
                    json(serde_json::json!({ "ok": true }))
                }
                Err(_) => status(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        _ => status(StatusCode::NOT_FOUND),
    }
}

async fn read(request: Request<Incoming>, max: usize) -> Option<Bytes> {
    let body = Limited::new(request.into_body(), max);
    body.collect()
        .await
        .ok()
        .map(|collected| collected.to_bytes())
}

fn script() -> Response<Body> {
    let mut response = Response::new(full(SCRIPT));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/javascript; charset=utf-8"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}
