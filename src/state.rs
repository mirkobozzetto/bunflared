use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::os;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub pid: u32,
    pub tunnel_pid: u32,
    pub url: String,
    pub routes: BTreeMap<String, u16>,
    pub started_at: u64,
    /// When `--for` runs out, in Unix seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    /// The `--password` visitors type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

impl Record {
    /// Held by a `bunflared mcp` process among others: its id is not its pid,
    /// and it stops when its record goes away, never on a signal.
    pub fn hosted(&self) -> bool {
        self.id != self.pid.to_string()
    }
}

fn dir() -> Option<PathBuf> {
    crate::os::data_dir()
}

pub fn file(id: &str) -> Option<PathBuf> {
    Some(dir()?.join(format!("{id}.json")))
}

/// The record of a live share on disk, removed when dropped.
pub struct Saved(Option<PathBuf>);

impl Saved {
    pub fn write(record: &Record) -> Self {
        let path = dir()
            .and_then(|dir| fs::create_dir_all(&dir).ok())
            .and_then(|_| file(&record.id));
        let written = path.filter(|path| {
            serde_json::to_vec(record).is_ok_and(|json| fs::write(path, json).is_ok())
        });
        Self(written)
    }
}

/// Whether the record of `id` is on disk; `down` removes it to stop a hosted share.
pub fn recorded(id: &str) -> bool {
    file(id).is_some_and(|path| path.exists())
}

impl Drop for Saved {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = fs::remove_file(path);
        }
    }
}

/// Live shares; records left by a dead process are deleted on the way.
pub fn live() -> Vec<Record> {
    let Some(entries) = dir().and_then(|dir| fs::read_dir(dir).ok()) else {
        return Vec::new();
    };
    let mut records: Vec<Record> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| {
            let record: Record = serde_json::from_slice(&fs::read(&path).ok()?).ok()?;
            if os::alive(record.pid) {
                Some(record)
            } else {
                let _ = fs::remove_file(&path);
                None
            }
        })
        .collect();
    records.sort_by_key(|record| record.started_at);
    records
}
