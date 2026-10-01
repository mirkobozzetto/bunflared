//! The shares a `bunflared mcp` process runs for its agent, and what happened
//! on them since the agent last looked.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use hyper::HeaderMap;
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::live::{Command, Hub, REACTIONS};
use crate::proxy::{self, CAPTURE, Capture};
use crate::share::{self, Event, Hit, Spec};
use crate::state::{self, Record};
use crate::widget::{Feedback, Presence};

const KEEP_EVENTS: usize = 1000;
const KEEP_REQUESTS: usize = 200;
/// A page that said goodbye and sent no new ping since has left; a visitor
/// moving to another page says goodbye too, then pings right away.
const GONE_GRACE: Duration = Duration::from_secs(3);
const STOP_GRACE: Duration = Duration::from_secs(1);
const TICK: Duration = Duration::from_millis(500);
const REPLAY_TIMEOUT: Duration = Duration::from_secs(30);
const ENDING_GRACE: Duration = Duration::from_secs(3);

pub struct Host {
    folder: Option<PathBuf>,
    shares: Mutex<Vec<Arc<Share>>>,
    opened: AtomicU32,
    /// Shares still digging their tunnel, out of `shares` until ready.
    opening: AtomicUsize,
    ending: AtomicBool,
    /// Held for the whole of `close_all`: stdin closing and a signal can both
    /// call it, and neither may exit while the other still stops shares.
    closing: Mutex<()>,
    log: Mutex<Log>,
    logged: Condvar,
}

#[derive(Default)]
struct Log {
    last: u64,
    events: VecDeque<Value>,
}

pub struct Share {
    record: Record,
    local: bool,
    hub: Arc<Hub>,
    runtime: tokio::runtime::Handle,
    stop: watch::Sender<bool>,
    thread: Mutex<Option<JoinHandle<()>>>,
    /// Whether `bunflared down` can reach it by removing its record.
    recorded: bool,
    seen: Mutex<Seen>,
}

#[derive(Default)]
struct Seen {
    visitors: HashMap<String, Visitor>,
    requests: VecDeque<Row>,
    total: u32,
}

struct Visitor {
    presence: Presence,
    seen: Instant,
    first: Instant,
    departed: bool,
}

struct Row {
    n: u32,
    hit: Hit,
    at: String,
}

impl Host {
    pub fn new(folder: Option<PathBuf>) -> Arc<Self> {
        let host = Arc::new(Self {
            folder,
            shares: Mutex::default(),
            opened: AtomicU32::new(0),
            opening: AtomicUsize::new(0),
            ending: AtomicBool::new(false),
            closing: Mutex::default(),
            log: Mutex::default(),
            logged: Condvar::new(),
        });
        let ticking = host.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(TICK);
                ticking.tick();
            }
        });
        host
    }

    /// Starts a share and returns once it answers, or with why it could not.
    pub fn open(self: &Arc<Self>, ports: Vec<u16>, public: bool) -> Result<Value, String> {
        self.opening.fetch_add(1, Ordering::SeqCst);
        let opened = self.start(ports, public);
        self.opening.fetch_sub(1, Ordering::SeqCst);
        opened
    }

    fn start(self: &Arc<Self>, ports: Vec<u16>, public: bool) -> Result<Value, String> {
        let n = self.opened.fetch_add(1, Ordering::Relaxed) + 1;
        let id = format!("{}-{n}", std::process::id());
        let (tx, rx) = mpsc::channel();
        let hub = Arc::new(Hub::new(tx.clone(), self.folder.clone()));
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|err| err.to_string())?;
        let handle = runtime.handle().clone();
        let (stop, mut stopped) = watch::channel(false);
        let spec = Spec {
            ports,
            local: !public,
            id: id.clone(),
            copy: false,
        };
        let (folder, served) = (self.folder.clone(), hub.clone());
        let thread = std::thread::spawn(move || {
            let result = runtime.block_on(async {
                tokio::select! {
                    result = share::serve(&spec, &tx, folder, served) => result,
                    _ = stopped.changed() => Ok(()),
                }
            });
            // Its own runtime: dropping it ends the proxy, the pages' sockets
            // and cloudflared with it.
            runtime.shutdown_timeout(STOP_GRACE);
            let _ = tx.send(Event::Done(result));
        });
        let ended = || "The session ended before the share was ready.".to_string();
        let record = loop {
            match rx.recv_timeout(TICK) {
                Ok(Event::Ready { record, .. }) => break record,
                Ok(Event::Done(result)) => {
                    let _ = thread.join();
                    return Err(result
                        .err()
                        .map_or("The share stopped before it was ready.".into(), |f| {
                            f.message
                        }));
                }
                Ok(_) => {}
                // The process is about to exit: cloudflared must not outlive it.
                Err(RecvTimeoutError::Timeout) if self.ending.load(Ordering::SeqCst) => {
                    let _ = stop.send(true);
                    let _ = thread.join();
                    return Err(ended());
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    return Err("The share stopped before it was ready.".into());
                }
            }
        };
        let share = Arc::new(Share {
            recorded: state::recorded(&id),
            record,
            local: !public,
            hub,
            runtime: handle,
            stop,
            thread: Mutex::new(Some(thread)),
            seen: Mutex::default(),
        });
        {
            let mut shares = self.shares.lock().unwrap();
            if self.ending.load(Ordering::SeqCst) {
                drop(shares);
                share.halt();
                return Err(ended());
            }
            shares.push(share.clone());
        }
        let host = self.clone();
        let summary = share.summary();
        std::thread::spawn(move || host.drain(&share, rx));
        Ok(summary)
    }

    pub fn list(&self) -> Value {
        let shares = self.shares.lock().unwrap();
        Value::Array(shares.iter().map(|share| share.summary()).collect())
    }

    pub fn close(&self, id: &str) -> Result<Value, String> {
        let share = {
            let mut shares = self.shares.lock().unwrap();
            let at = shares
                .iter()
                .position(|share| share.record.id == id)
                .ok_or_else(|| self.unknown(&shares, id))?;
            shares.remove(at)
        };
        share.halt();
        Ok(json!({ "closed": id }))
    }

    /// Closes every share, those still opening too, for the end of the session.
    pub fn close_all(&self) {
        let _closing = self.closing.lock().unwrap();
        self.ending.store(true, Ordering::SeqCst);
        let shares = std::mem::take(&mut *self.shares.lock().unwrap());
        for share in &shares {
            let _ = share.stop.send(true);
        }
        for share in &shares {
            share.halt();
        }
        let deadline = Instant::now() + ENDING_GRACE;
        while self.opening.load(Ordering::SeqCst) > 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// The share with this id, or the last one opened.
    pub fn share(&self, id: Option<&str>) -> Result<Arc<Share>, String> {
        let shares = self.shares.lock().unwrap();
        match id {
            Some(id) => shares
                .iter()
                .find(|share| share.record.id == id)
                .cloned()
                .ok_or_else(|| self.unknown(&shares, id)),
            None => shares
                .last()
                .cloned()
                .ok_or_else(|| "No share is open: open one first.".into()),
        }
    }

    fn unknown(&self, shares: &[Arc<Share>], id: &str) -> String {
        let open: Vec<&str> = shares.iter().map(|s| s.record.id.as_str()).collect();
        match open.len() {
            0 => format!("No open share with id {id}: none is open."),
            _ => format!("No open share with id {id}. Open: {}.", open.join(", ")),
        }
    }

    /// What happened after `since`, waiting up to `wait` for something when
    /// nothing did yet.
    pub fn events(&self, since: u64, wait: Duration, share: Option<&str>) -> Value {
        let deadline = Instant::now() + wait;
        let mut log = self.log.lock().unwrap();
        // A cursor from before a restart of this server starts over.
        let since = if since > log.last { 0 } else { since };
        loop {
            let found: Vec<Value> = log
                .events
                .iter()
                .filter(|event| event["seq"].as_u64() > Some(since))
                .filter(|event| share.is_none_or(|id| event["share"] == id))
                .cloned()
                .collect();
            let left = deadline.saturating_duration_since(Instant::now());
            if !found.is_empty() || left.is_zero() {
                let cursor = found
                    .last()
                    .and_then(|event| event["seq"].as_u64())
                    .unwrap_or(since);
                return json!({ "events": found, "cursor": cursor });
            }
            log = self.logged.wait_timeout(log, left).unwrap().0;
        }
    }

    fn log(&self, share: &str, kind: &str, mut event: Value) {
        let mut log = self.log.lock().unwrap();
        log.last += 1;
        event["seq"] = json!(log.last);
        event["share"] = json!(share);
        event["kind"] = json!(kind);
        event["at"] = json!(clock());
        log.events.push_back(event);
        if log.events.len() > KEEP_EVENTS {
            log.events.pop_front();
        }
        self.logged.notify_all();
    }

    /// Sends what a visitor wrote into the agent's session, for clients
    /// that listen to channels; the others drop it.
    fn channel(&self, content: &str, meta: Value) {
        super::notify(
            "notifications/claude/channel",
            json!({ "content": content, "meta": meta }),
        );
    }

    /// Departures, and shares that `bunflared down` stopped.
    fn tick(&self) {
        let shares = self.shares.lock().unwrap().clone();
        for share in shares {
            let id = &share.record.id;
            if share.recorded && !state::recorded(id) {
                self.log(
                    id,
                    "closed",
                    json!({ "reason": "stopped by bunflared down" }),
                );
                let _ = self.close(id);
                continue;
            }
            let left = share.seen.lock().unwrap().departures();
            for presence in left {
                self.log(id, "left", visitor(&presence));
            }
        }
    }

    fn drain(&self, share: &Share, rx: Receiver<Event>) {
        let id = share.record.id.as_str();
        for event in rx {
            match event {
                Event::Request(hit) => share.seen.lock().unwrap().request(hit),
                Event::Presence(presence) => {
                    if share.seen.lock().unwrap().presence(presence.clone()) {
                        self.log(id, "arrived", visitor(&presence));
                    }
                }
                Event::Chat(said) => {
                    self.log(
                        id,
                        "message",
                        json!({
                            "visitor": said.sid,
                            "device": said.device,
                            "page": said.page,
                            "text": said.text,
                        }),
                    );
                    let meta = json!({
                        "kind": "message",
                        "share": id,
                        "visitor": said.sid,
                        "device": said.device,
                        "page": said.page,
                    });
                    self.channel(&said.text, meta);
                }
                Event::Feedback(note) => self.noted(id, note),
                Event::Reacted(reaction) => {
                    let (emoji, label) = REACTIONS[reaction.kind];
                    let reacted =
                        json!({ "device": reaction.device, "emoji": emoji, "label": label });
                    self.log(id, "reaction", reacted);
                }
                Event::Done(result) => {
                    let mut shares = self.shares.lock().unwrap();
                    if let Some(at) = shares.iter().position(|s| s.record.id == id) {
                        shares.remove(at);
                        drop(shares);
                        let reason = result.err().map_or("stopped".into(), |f| f.message);
                        self.log(id, "closed", json!({ "reason": reason }));
                    }
                    return;
                }
                _ => {}
            }
        }
    }

    fn noted(&self, id: &str, note: Feedback) {
        let path = |path: &PathBuf| path.display().to_string();
        let element = note
            .element
            .as_ref()
            .map(|e| json!({ "selector": e.selector, "text": e.text }));
        self.log(
            id,
            "note",
            json!({
                "visitor": note.sid,
                "device": note.device,
                "page": note.page,
                "text": note.message,
                "file": path(&note.file),
                "screenshot": note.shot.as_ref().map(path),
                "element": element,
            }),
        );
        let mut meta = json!({
            "kind": "note",
            "share": id,
            "visitor": note.sid,
            "device": note.device,
            "page": note.page,
            "file": path(&note.file),
        });
        if let Some(shot) = &note.shot {
            meta["screenshot"] = json!(path(shot));
        }
        if let Some(element) = &note.element {
            meta["element"] = json!(element.selector);
        }
        self.channel(&note.message, meta);
    }
}

impl Share {
    fn halt(&self) {
        let _ = self.stop.send(true);
        if let Some(thread) = self.thread.lock().unwrap().take() {
            let _ = thread.join();
        }
    }

    fn summary(&self) -> Value {
        let seen = self.seen.lock().unwrap();
        let here = seen.visitors.values().filter(|v| !v.departed).count();
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
            "uptime": state::uptime(now.saturating_sub(self.record.started_at)),
        })
    }

    pub fn visitors(&self) -> Value {
        let seen = self.seen.lock().unwrap();
        let mut here: Vec<&Visitor> = seen.visitors.values().filter(|v| !v.departed).collect();
        here.sort_by_key(|v| v.first);
        let here = here
            .into_iter()
            .map(|v| {
                let mut entry = visitor(&v.presence);
                entry["status"] = json!(v.presence.status(v.seen.elapsed()));
                entry["here_for"] = json!(state::uptime(v.first.elapsed().as_secs()));
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
        let path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        };
        reached(self.hub.send(to, &Command::Go { path }))
    }

    pub fn reload(&self, to: Option<&str>) -> Result<Value, String> {
        reached(self.hub.send(to, &Command::Reload))
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
        let replay = match refusal(&hit) {
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
        if let Some(reason) = refusal(&hit) {
            return Err(format!("Not replayed: {reason}"));
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
    fn request(&mut self, hit: Hit) {
        self.total += 1;
        let (n, at) = (self.total, clock());
        self.requests.push_front(Row { n, hit, at });
        self.requests.truncate(KEEP_REQUESTS);
    }

    /// Keeps the visitor's last ping; true when they just arrived.
    fn presence(&mut self, presence: Presence) -> bool {
        let now = Instant::now();
        let known = self.visitors.get(&presence.sid).filter(|v| !v.departed);
        let arrived = known.is_none() && !presence.gone;
        if known.is_none() && presence.gone {
            return false;
        }
        let first = known.map_or(now, |v| v.first);
        let sid = presence.sid.clone();
        self.visitors.insert(
            sid,
            Visitor {
                presence,
                seen: now,
                first,
                departed: false,
            },
        );
        arrived
    }

    fn departures(&mut self) -> Vec<Presence> {
        let mut left = Vec::new();
        for v in self.visitors.values_mut().filter(|v| !v.departed) {
            let away = v.seen.elapsed();
            if (v.presence.gone && away >= GONE_GRACE) || v.presence.status(away) == "left" {
                v.departed = true;
                left.push(v.presence.clone());
            }
        }
        left
    }
}

fn visitor(presence: &Presence) -> Value {
    json!({
        "visitor": presence.sid,
        "device": presence.device,
        "page": presence.page,
    })
}

fn reached(pages: usize) -> Result<Value, String> {
    match pages {
        0 => Err("No page is connected live: nobody got it.".into()),
        n => Ok(json!({ "pages": n })),
    }
}

/// Why a request cannot be sent again as it was, if it cannot.
fn refusal(hit: &Hit) -> Option<String> {
    if hit.upgrade {
        Some("a WebSocket cannot be sent again.".into())
    } else if !hit.exchange.request_body.lock().unwrap().complete() {
        Some(format!(
            "its body is over {} KiB and only the start was kept.",
            CAPTURE / 1024
        ))
    } else {
        None
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

fn clock() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}
