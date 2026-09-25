use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub pid: u32,
    pub tunnel_pid: u32,
    pub url: String,
    pub routes: BTreeMap<String, u16>,
    pub started_at: u64,
}

fn dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
        })?;
    Some(base.join("bunflared"))
}

fn file(id: &str) -> Option<PathBuf> {
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

impl Drop for Saved {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = fs::remove_file(path);
        }
    }
}
