//! The agent looking at a visitor's page: each request waits here for the
//! page's answer, the screenshot it saved and what it had on screen.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use serde::Deserialize;

pub const NOBODY: &str = "No page is connected live: nobody got it.";

/// What a page answers, or why it could not.
#[derive(Debug, Deserialize)]
pub struct Looked {
    pub id: u64,
    #[serde(default)]
    pub shot: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub page: String,
    #[serde(default)]
    pub screen: String,
    #[serde(default)]
    pub text: String,
    /// The element asked for was not on the page.
    #[serde(default)]
    pub missing: bool,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Default)]
pub struct Looks {
    next: AtomicU64,
    waiting: Mutex<HashMap<u64, Sender<Looked>>>,
}

impl Looks {
    /// Hands a fresh id to `send`, then waits for the answer carrying it.
    pub fn ask(&self, wait: Duration, send: impl FnOnce(u64) -> bool) -> Result<Looked, String> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, answer) = mpsc::channel();
        self.waiting.lock().unwrap().insert(id, tx);
        let looked = if send(id) {
            answer
                .recv_timeout(wait)
                .map_err(|_| "The page did not answer in time.".to_string())
        } else {
            Err(NOBODY.to_string())
        };
        self.waiting.lock().unwrap().remove(&id);
        looked
    }

    /// An answer nobody waits for any more is dropped.
    pub fn answer(&self, looked: Looked) {
        if let Some(tx) = self.waiting.lock().unwrap().remove(&looked.id) {
            let _ = tx.send(looked);
        }
    }
}
