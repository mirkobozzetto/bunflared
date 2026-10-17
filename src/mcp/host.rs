//! The shares a `bunflared mcp` process runs for its agent, and what happened
//! on them since the agent last looked.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::sync::watch;

use crate::event::Event;
use crate::live::{Hub, REACTIONS};
use crate::notes::Feedback;
use crate::share::{self, Spec};
use crate::state;

use super::journal::Journal;
use super::share::{Share, visitor};

const STOP_GRACE: Duration = Duration::from_secs(1);
const TICK: Duration = Duration::from_millis(500);
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
    journal: Journal,
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
            journal: Journal::default(),
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
    pub fn open(
        self: &Arc<Self>,
        ports: Vec<u16>,
        public: bool,
        expires_in: Option<Duration>,
    ) -> Result<Value, String> {
        self.opening.fetch_add(1, Ordering::SeqCst);
        let opened = self.start(ports, public, expires_in);
        self.opening.fetch_sub(1, Ordering::SeqCst);
        opened
    }

    fn start(
        self: &Arc<Self>,
        ports: Vec<u16>,
        public: bool,
        expires_in: Option<Duration>,
    ) -> Result<Value, String> {
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
            expires_in,
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
            folder: self.folder.clone(),
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

    pub fn events(&self, since: u64, wait: Duration, share: Option<&str>) -> Value {
        self.journal.events(since, wait, share)
    }

    /// Answers the `events` calls still waiting with what they have.
    pub fn end_waits(&self) {
        self.journal.end();
    }

    fn log(&self, share: &str, kind: &str, event: Value) {
        self.journal.log(share, kind, event);
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
            let left = share.seen.lock().unwrap().visitors.departures();
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
                    let arrived = share
                        .seen
                        .lock()
                        .unwrap()
                        .visitors
                        .update(presence.clone(), Instant::now());
                    if arrived {
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
                Event::PageError(error) => {
                    self.log(
                        id,
                        "error",
                        json!({
                            "visitor": error.sid,
                            "device": error.device,
                            "page": error.page,
                            "type": error.kind,
                            "message": error.message,
                            "source": error.source,
                            "stack": error.stack,
                            "count": error.count,
                        }),
                    );
                    let meta = json!({
                        "kind": "error",
                        "share": id,
                        "visitor": error.sid,
                        "page": error.page,
                        "source": error.source,
                        "count": error.count,
                    });
                    self.channel(&error.message, meta);
                }
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
        // `element` stays the first point, for agents that read it already.
        let element = note
            .points
            .first()
            .map(|e| json!({ "selector": e.selector, "text": e.text, "source": e.source }));
        let points: Vec<Value> = note
            .points
            .iter()
            .map(|e| {
                json!({ "selector": e.selector, "text": e.text, "comment": e.comment, "source": e.source })
            })
            .collect();
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
                "points": points,
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
        if let Some(element) = note.points.first() {
            meta["element"] = json!(element.selector);
            if let Some(source) = &element.source {
                meta["source"] = json!(source.at());
            }
        }
        self.channel(&note.text(), meta);
    }
}
