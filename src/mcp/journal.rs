//! What happened on the shares, numbered, so the agent reads it from a cursor
//! and can wait for the next thing.

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::clock::clock;

const KEEP_EVENTS: usize = 1000;

#[derive(Default)]
pub struct Journal {
    log: Mutex<Log>,
    logged: Condvar,
}

#[derive(Default)]
struct Log {
    last: u64,
    events: VecDeque<Value>,
}

impl Journal {
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

    pub fn log(&self, share: &str, kind: &str, mut event: Value) {
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
}
