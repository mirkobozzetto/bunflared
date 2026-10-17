//! One share as the agent sees it: its visitors and requests, and what the
//! agent sends its pages.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use hyper::HeaderMap;
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::clock::{clock, uptime};
use crate::event::{Capture, Hit};
use crate::live::{self, Command, Hub};
use crate::look::NOBODY;
use crate::notes;
use crate::proxy;
use crate::state::Record;
use crate::visitors::{Presence, Roster};

const KEEP_REQUESTS: usize = 200;
const REPLAY_TIMEOUT: Duration = Duration::from_secs(30);
const LOOK_WAIT: Duration = Duration::from_secs(15);
// On a public link the visitor is asked first, and gets time to answer.
const ASK_WAIT: Duration = Duration::from_secs(60);
const DECLINED: &str = "declined";

pub struct Share {
    pub(super) record: Record,
    pub(super) local: bool,
    /// Where the pages save their screenshots.
    pub(super) folder: Option<PathBuf>,
    pub(super) hub: Arc<Hub>,
    pub(super) runtime: tokio::runtime::Handle,
    pub(super) stop: watch::Sender<bool>,
    pub(super) thread: Mutex<Option<JoinHandle<()>>>,
    /// Whether `bunflared down` can reach it by removing its record.
    pub(super) recorded: bool,
    pub(super) seen: Mutex<Seen>,
}

#[derive(Default)]
pub struct Seen {
    pub(super) visitors: Roster,
    requests: VecDeque<Row>,
    total: u32,
}

struct Row {
    n: u32,
    hit: Hit,
    at: String,
}

impl Share {
    pub fn halt(&self) {
        let _ = self.stop.send(true);
        if let Some(thread) = self.thread.lock().unwrap().take() {
            let _ = thread.join();
        }
    }

    pub fn summary(&self) -> Value {
        let seen = self.seen.lock().unwrap();
        let here = seen.visitors.len();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        json!({
            "id": self.record.id,
            "url": self.record.url,
            "routes": self.record.routes,
            "local": self.local,
            "visitors": here,
            "requests": seen.total,
            "uptime": uptime(now.saturating_sub(self.record.started_at)),
        })
    }

    pub fn visitors(&self) -> Value {
        let seen = self.seen.lock().unwrap();
        let here = seen
            .visitors
            .by_arrival()
            .into_iter()
            .map(|v| {
                let mut entry = visitor(&v.presence);
                entry["status"] = json!(v.presence.status(v.seen.elapsed()));
                entry["here_for"] = json!(uptime(v.first.elapsed().as_secs()));
                entry
            })
            .collect();
        Value::Array(here)
    }

    pub fn say(&self, to: Option<&str>, text: &str) -> Result<Value, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("Nothing to say: pass the text.".into());
        }
        let name = to.map_or_else(|| "everyone".into(), |sid| self.device(sid));
        reached(self.hub.say(to, &name, text))
    }

    pub fn go(&self, to: Option<&str>, path: &str) -> Result<Value, String> {
        let path = path.trim();
        if path.is_empty() {
            return Err("Pass the path to send them to.".into());
        }
        let path = live::absolute(path);
        reached(self.hub.send(to, &Command::Go { path }))
    }

    pub fn reload(&self, to: Option<&str>) -> Result<Value, String> {
        reached(self.hub.send(to, &Command::Reload))
    }

    /// A screenshot of what a visitor sees, or of one element after a
    /// reload, with what was on screen.
    pub fn look(
        &self,
        to: Option<&str>,
        selector: Option<&str>,
        reload: bool,
    ) -> Result<Value, String> {
        let sid = match to {
            Some(sid) => sid.to_string(),
            None => self.only_visitor()?,
        };
        let wait = if self.local { LOOK_WAIT } else { ASK_WAIT };
        let looked = self.hub.look(&sid, selector, reload, wait)?;
        match looked.error.as_deref() {
            Some(DECLINED) => return Err("The visitor said no: their page stays private.".into()),
            Some(error) => return Err(format!("The page could not take the screenshot: {error}")),
            None => {}
        }
        let folder = self.folder.as_ref().ok_or("No folder to keep screenshots in.")?;
        let shot = looked
            .shot
            .filter(|name| notes::kept(folder, name))
            .ok_or("The page sent no screenshot.")?;
        let mut seen = json!({
            "visitor": sid,
            "page": looked.page,
            "title": looked.title,
            "screen": looked.screen,
            "text": looked.text,
            "screenshot": folder.join(shot).display().to_string(),
        });
        if selector.is_some() {
            seen["found"] = json!(!looked.missing);
        }
        Ok(seen)
    }

    /// The one visitor connected live, when there is one.
    fn only_visitor(&self) -> Result<String, String> {
        let sids = self.hub.connected();
        match sids.as_slice() {
            [] => Err(NOBODY.into()),
            [sid] => Ok(sid.clone()),
            _ => {
                let named: Vec<String> = sids
                    .iter()
                    .map(|sid| format!("{sid} ({})", self.device(sid)))
                    .collect();
                Err(format!(
                    "Several visitors are here: pass visitor, one of {}.",
                    named.join(", ")
                ))
            }
        }
    }

    pub fn requests(&self, limit: usize) -> Value {
        let seen = self.seen.lock().unwrap();
        let rows = seen
            .requests
            .iter()
            .take(limit)
            .map(|row| {
                json!({
                    "n": row.n,
                    "method": row.hit.method,
                    "path": row.hit.path,
                    "status": row.hit.status,
                    "ms": row.hit.ms,
                    "visitor": row.hit.visitor,
                    "at": row.at,
                })
            })
            .collect();
        Value::Array(rows)
    }

    pub fn request(&self, n: u32) -> Result<Value, String> {
        let (hit, at) = self.row(n)?;
        let exchange = &hit.exchange;
        let replay = match hit.unreplayable() {
            Some(reason) => json!(reason),
            None => json!(true),
        };
        let request = exchange.request_body.lock().unwrap();
        let response = exchange.response_body.lock().unwrap();
        Ok(json!({
            "n": n,
            "method": hit.method,
            "path": hit.path,
            "status": hit.status,
            "ms": hit.ms,
            "port": hit.port,
            "visitor": hit.visitor,
            "at": at,
            "replayable": replay,
            "request": {
                "headers": headers(&exchange.request_headers),
                "body": body(&exchange.request_headers, &request),
            },
            "response": {
                "headers": headers(&exchange.response_headers),
                "body": body(&exchange.response_headers, &response),
            },
        }))
    }

    pub fn replay(&self, n: u32) -> Result<Value, String> {
        let (hit, _) = self.row(n)?;
        if let Some(reason) = hit.unreplayable() {
            return Err(format!("Not replayed. {reason}"));
        }
        let started = Instant::now();
        let status = self
            .runtime
            .block_on(async {
                tokio::time::timeout(
                    REPLAY_TIMEOUT,
                    proxy::replay(&hit.method, &hit.path, hit.port, &hit.exchange),
                )
                .await
            })
            .map_err(|_| "The local server did not answer in time.".to_string())?
            .map_err(|err| format!("The replay failed: {err}"))?;
        Ok(json!({
            "n": n,
            "before": hit.status,
            "status": status,
            "ms": started.elapsed().as_millis(),
        }))
    }

    fn row(&self, n: u32) -> Result<(Hit, String), String> {
        let seen = self.seen.lock().unwrap();
        seen.requests
            .iter()
            .find(|row| row.n == n)
            .map(|row| (row.hit.clone(), row.at.clone()))
            .ok_or_else(|| format!("No request {n} in the last {KEEP_REQUESTS}."))
    }

    fn device(&self, sid: &str) -> String {
        let seen = self.seen.lock().unwrap();
        seen.visitors
            .get(sid)
            .map_or_else(|| sid.to_string(), |v| v.presence.device.clone())
    }
}

impl Seen {
    pub fn request(&mut self, hit: Hit) {
        self.total += 1;
        let (n, at) = (self.total, clock());
        self.requests.push_front(Row { n, hit, at });
        self.requests.truncate(KEEP_REQUESTS);
    }
}

pub fn visitor(presence: &Presence) -> Value {
    json!({
        "visitor": presence.sid,
        "device": presence.device,
        "page": presence.page,
    })
}

fn reached(pages: usize) -> Result<Value, String> {
    match pages {
        0 => Err(NOBODY.into()),
        n => Ok(json!({ "pages": n })),
    }
}

fn headers(headers: &HeaderMap) -> Vec<String> {
    headers
        .iter()
        .map(|(name, value)| format!("{name}: {}", String::from_utf8_lossy(value.as_bytes())))
        .collect()
}

/// A body as the dashboard shows it: the kept start when it is text.
fn body(headers: &HeaderMap, capture: &Capture) -> Value {
    if capture.total == 0 {
        return json!({ "size": 0 });
    }
    if !capture.readable(headers) {
        return json!({ "size": capture.total, "text": null, "note": "not text, not shown" });
    }
    json!({
        "size": capture.total,
        "text": String::from_utf8_lossy(&capture.bytes),
        "truncated": !capture.complete(),
    })
}
