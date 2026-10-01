//! The companion script injected into shared pages: live presence for the
//! dashboard, and a feedback button whose notes and screenshots are saved in
//! the folder bunflared runs from.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Limited};
use hyper::body::Incoming;
use hyper::header::{self, HeaderValue};
use hyper::{Method, Request, Response, StatusCode};
use serde::Deserialize;

use crate::live::{self, Hub};
use crate::proxy::{Body, full, status};
use crate::share::{Event, Tx};

pub const PREFIX: &str = "/_bunflared/";
pub const FOLDER: &str = "bunflared-feedback";
const SCRIPT: &str = include_str!("widget.js");
const TAG: &str = r#"<script src="/_bunflared/widget.js" defer></script>"#;
const MAX_PING: usize = 4 * 1024;
const MAX_NOTE: usize = 64 * 1024;
const MAX_SHOT: usize = 12 * 1024 * 1024;
const MAX_PINNED: usize = 300;
const LEFT_AFTER: Duration = Duration::from_secs(20);
// A hidden tab's timers wake up once a minute in Chrome, every 40 s in
// Safari: its pings slow down without it leaving.
const HIDDEN_LEFT_AFTER: Duration = Duration::from_secs(90);
const IDLE_AFTER: u64 = 30;

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

impl Presence {
    /// "active", "tab hidden", "idle 45s" or "left", `away` after its last ping.
    pub fn status(&self, away: Duration) -> String {
        let idle = u64::from(self.idle) + away.as_secs();
        let patience = if self.visible {
            LEFT_AFTER
        } else {
            HIDDEN_LEFT_AFTER
        };
        if self.gone || away > patience {
            "left".into()
        } else if !self.visible {
            "tab hidden".into()
        } else if idle >= IDLE_AFTER {
            format!("idle {idle}s")
        } else {
            "active".into()
        }
    }
}

#[derive(Debug, Clone)]
pub struct Feedback {
    pub sid: String,
    pub device: String,
    pub page: String,
    pub message: String,
    /// The note's Markdown file, and its screenshot.
    pub file: PathBuf,
    pub shot: Option<PathBuf>,
    pub element: Option<Element>,
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
    element: Option<Element>,
}

/// The element a visitor pointed at, so whoever reads the note finds it.
#[derive(Debug, Clone, Deserialize)]
pub struct Element {
    pub selector: String,
    pub text: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Element {
    fn markdown(&self) -> String {
        // Browser text goes inside backticks and quotes: keep it on one line
        // and unable to close them.
        let clean = |text: &str| -> String {
            text.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .replace(['`', '"'], "'")
                .chars()
                .take(MAX_PINNED)
                .collect()
        };
        let mut lines = format!("- Element: `{}`\n", clean(&self.selector));
        if !self.text.trim().is_empty() {
            lines.push_str(&format!("- Text: \"{}\"\n", clean(&self.text)));
        }
        lines.push_str(&format!(
            "- Position: {}, {} ({}×{}) from the top left of the page\n",
            self.x.round(),
            self.y.round(),
            self.w.round(),
            self.h.round()
        ));
        lines
    }
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
                .filter(|note| !note.message.trim().is_empty());
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
                        element: note.element,
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
pub fn folder_ready(folder: &Path) -> std::io::Result<()> {
    fs::create_dir_all(folder)?;
    let ignore = folder.join(".gitignore");
    if !ignore.exists() {
        fs::write(ignore, "# Client feedback collected by bunflared.\n*\n")?;
    }
    Ok(())
}

pub fn stamp() -> String {
    chrono::Local::now()
        .format("%Y-%m-%d_%H-%M-%S%.3f")
        .to_string()
}

/// The time of day, as the logs show it.
pub fn clock() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
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

/// Returns the note's file name, and its screenshot's.
fn save_note(
    folder: &Path,
    note: &Note,
    device: &str,
) -> std::io::Result<(String, Option<String>)> {
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
    let pinned = note.element.as_ref().map(Element::markdown);
    let mut text = format!(
        "# Feedback, {}\n\n- Page: `{}`\n- Device: {device}\n- Screen: {}\n- Visitor: {}\n{}\n{}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        note.page,
        note.screen.as_deref().unwrap_or("unknown"),
        note.sid,
        pinned.unwrap_or_default(),
        quoted.join("\n"),
    );
    if let Some(shot) = shot {
        let outlined = if note.element.is_some() {
            ", the element outlined"
        } else {
            ""
        };
        text.push_str(&format!("\n![Screenshot{outlined}]({shot})\n"));
    }
    let name = format!("{}.md", stamp());
    fs::write(folder.join(&name), text)?;
    Ok((name, shot.map(str::to_string)))
}

/// "iPhone · Safari", from the User-Agent and the `Sec-CH-UA` client hint.
pub fn device(agent: &str, hints: &str) -> String {
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
    // Brave reads as Safari on iOS and as Chrome elsewhere, but names itself
    // at the end of the iOS agent and in the client hint.
    let brave = agent.contains("Brave") || hints.contains("\"Brave\"");
    let browser = [
        ("Edg/", "Edge"),
        ("EdgiOS/", "Edge"),
        ("OPR/", "Opera"),
        ("OPT/", "Opera"),
        ("Firefox/", "Firefox"),
        ("FxiOS/", "Firefox"),
        ("CriOS/", "Chrome"),
        ("Chrome/", "Chrome"),
        ("Safari/", "Safari"),
    ]
    .into_iter()
    .find(|(needle, _)| agent.contains(needle))
    .map_or("browser", |(_, name)| name);
    let browser = if brave { "Brave" } else { browser };
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
