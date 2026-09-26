//! The companion script injected into shared pages: live presence for the
//! dashboard, and a feedback button whose notes and screenshots are saved in
//! the folder bunflared runs from.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{BodyExt, Limited};
use hyper::body::Incoming;
use hyper::header::{self, HeaderValue};
use hyper::{Method, Request, Response, StatusCode};
use serde::Deserialize;

use crate::live::{self, Hub};
use crate::proxy::{Body, full};
use crate::share::{Event, Tx};

pub const PREFIX: &str = "/_bunflared/";
pub const FOLDER: &str = "bunflared-feedback";
const SCRIPT: &str = include_str!("widget.js");
const TAG: &str = r#"<script src="/_bunflared/widget.js" defer></script>"#;
const MAX_PING: usize = 4 * 1024;
const MAX_NOTE: usize = 64 * 1024;
const MAX_SHOT: usize = 12 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Presence {
    pub sid: String,
    pub device: String,
    pub page: String,
    pub visible: bool,
    pub idle: u32,
    pub clicks: u32,
    pub gone: bool,
}

#[derive(Debug, Clone)]
pub struct Feedback {
    pub device: String,
    pub page: String,
    pub message: String,
}

#[derive(Deserialize)]
struct Ping {
    sid: String,
    page: String,
    visible: bool,
    idle: u32,
    clicks: u32,
    gone: bool,
}

#[derive(Deserialize)]
struct Note {
    sid: String,
    page: String,
    message: String,
    screen: Option<String>,
    shot: Option<String>,
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
    let device = device(
        request
            .headers()
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
    );
    let route = request.uri().path().trim_start_matches(PREFIX).to_string();
    let method = request.method().clone();
    match (method, route.as_str()) {
        (Method::GET, "widget.js") => script(),
        (Method::GET, "live") => live::accept(request, hub.clone()),
        (Method::POST, "ping") => match read(request, MAX_PING).await {
            Some(body) => {
                if let Ok(ping) = serde_json::from_slice::<Ping>(&body) {
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
                .filter(|note| !note.message.trim().is_empty());
            let Some(note) = note else {
                return status(StatusCode::BAD_REQUEST);
            };
            match save_note(folder, &note, &device) {
                Ok(()) => {
                    let _ = tx.send(Event::Feedback(Feedback {
                        device,
                        page: note.page,
                        message: note.message.trim().to_string(),
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

/// The feedback folder, created with a `.gitignore` of its own so no project
/// ever commits a client's notes by accident.
fn folder_ready(folder: &Path) -> std::io::Result<()> {
    fs::create_dir_all(folder)?;
    let ignore = folder.join(".gitignore");
    if !ignore.exists() {
        fs::write(ignore, "# Client feedback collected by bunflared.\n*\n")?;
    }
    Ok(())
}

fn stamp() -> String {
    chrono::Local::now()
        .format("%Y-%m-%d_%H-%M-%S%.3f")
        .to_string()
}

fn save_shot(folder: &Path, image: &[u8]) -> std::io::Result<String> {
    let extension = match image {
        [0x89, b'P', b'N', b'G', ..] => "png",
        [0xFF, 0xD8, 0xFF, ..] => "jpg",
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => "webp",
        _ => return Err(std::io::ErrorKind::InvalidData.into()),
    };
    folder_ready(folder)?;
    let name = format!("{}.{extension}", stamp());
    fs::write(folder.join(&name), image)?;
    Ok(name)
}

fn save_note(folder: &Path, note: &Note, device: &str) -> std::io::Result<()> {
    folder_ready(folder)?;
    // Only a name this module gave out, never a path from the browser.
    let shot = note.shot.as_deref().filter(|name| {
        name.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            && !name.contains("..")
            && folder.join(name).is_file()
    });
    let quoted: Vec<String> = note
        .message
        .trim()
        .lines()
        .map(|line| format!("> {line}"))
        .collect();
    let mut text = format!(
        "# Feedback, {}\n\n- Page: `{}`\n- Device: {device}\n- Screen: {}\n- Visitor: {}\n\n{}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        note.page,
        note.screen.as_deref().unwrap_or("unknown"),
        note.sid,
        quoted.join("\n"),
    );
    if let Some(shot) = shot {
        text.push_str(&format!("\n![Screenshot]({shot})\n"));
    }
    fs::write(folder.join(format!("{}.md", stamp())), text)
}

/// "iPhone · Safari", from the User-Agent header.
pub fn device(agent: &str) -> String {
    let system = [
        ("iPhone", "iPhone"),
        ("iPad", "iPad"),
        ("Android", "Android"),
        ("Macintosh", "Mac"),
        ("Windows", "Windows"),
        ("Linux", "Linux"),
    ]
    .into_iter()
    .find(|(needle, _)| agent.contains(needle))
    .map_or("Unknown", |(_, name)| name);
    let browser = [
        ("Edg/", "Edge"),
        ("OPR/", "Opera"),
        ("Firefox/", "Firefox"),
        ("FxiOS/", "Firefox"),
        ("CriOS/", "Chrome"),
        ("Chrome/", "Chrome"),
        ("Safari/", "Safari"),
    ]
    .into_iter()
    .find(|(needle, _)| agent.contains(needle))
    .map_or("browser", |(_, name)| name);
    format!("{system} · {browser}")
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

fn json(value: serde_json::Value) -> Response<Body> {
    let mut response = Response::new(full(value.to_string()));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response
}

fn status(code: StatusCode) -> Response<Body> {
    let mut response = Response::new(full(Bytes::new()));
    *response.status_mut() = code;
    response
}
