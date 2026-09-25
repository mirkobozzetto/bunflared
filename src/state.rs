use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const DOWN_GRACE: Duration = Duration::from_secs(3);

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

fn alive(pid: u32) -> bool {
    pid != 0 && unsafe { libc::kill(pid as i32, 0) } == 0
}

/// Live shares; records left by a dead process are deleted on the way.
fn live() -> Vec<Record> {
    let Some(entries) = dir().and_then(|dir| fs::read_dir(dir).ok()) else {
        return Vec::new();
    };
    let mut records: Vec<Record> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| {
            let record: Record = serde_json::from_slice(&fs::read(&path).ok()?).ok()?;
            if alive(record.pid) {
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

pub fn list(json: bool) -> i32 {
    let records = live();
    if json {
        println!(
            "{}",
            serde_json::to_string(&records).unwrap_or_else(|_| "[]".into())
        );
        return 0;
    }
    if records.is_empty() {
        println!("No live shares.");
        return 0;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    println!("{:<8} {:<64} {:<16} UPTIME", "ID", "URL", "PORTS");
    for record in records {
        let ports: Vec<String> = record.routes.values().map(u16::to_string).collect();
        println!(
            "{:<8} {:<64} {:<16} {}",
            record.id,
            record.url,
            ports.join(","),
            uptime(now.saturating_sub(record.started_at))
        );
    }
    0
}

pub fn uptime(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs}s"),
        60..3600 => format!("{}m {:02}s", secs / 60, secs % 60),
        _ => format!("{}h {:02}m", secs / 3600, secs % 3600 / 60),
    }
}

pub fn down(id: Option<String>, all: bool) -> i32 {
    let targets: Vec<Record> = live()
        .into_iter()
        .filter(|record| all || id.as_deref() == Some(record.id.as_str()))
        .collect();
    if targets.is_empty() {
        match id {
            Some(id) if !all => eprintln!("No live share with id {id}. See: bunflared ls"),
            _ => println!("No live shares."),
        }
        return if all { 0 } else { 1 };
    }
    for record in &targets {
        unsafe { libc::kill(record.pid as i32, libc::SIGTERM) };
    }
    let deadline = Instant::now() + DOWN_GRACE;
    while Instant::now() < deadline && targets.iter().any(|record| alive(record.pid)) {
        std::thread::sleep(Duration::from_millis(50));
    }
    for record in &targets {
        if alive(record.pid) {
            unsafe {
                libc::kill(record.pid as i32, libc::SIGKILL);
                libc::kill(record.tunnel_pid as i32, libc::SIGKILL);
            }
            if let Some(path) = file(&record.id) {
                let _ = fs::remove_file(path);
            }
        }
        println!("Stopped {} {}", record.id, record.url);
    }
    0
}

/// Starts the share in its own session and returns once it prints its ready
/// line, so the caller's shell is free while the share keeps running.
pub fn detach(ports: &[u16]) -> i32 {
    let Ok(exe) = std::env::current_exe() else {
        eprintln!(r#"{{"error":"cannot find the bunflared executable","code":1}}"#);
        return 1;
    };
    let mut command = Command::new(exe);
    command
        .args(ports.iter().map(u16::to_string))
        .arg("--json")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            eprintln!(
                "{}",
                serde_json::json!({ "error": err.to_string(), "code": 1 })
            );
            return 1;
        }
    };
    let mut line = String::new();
    if let Some(stdout) = child.stdout.take() {
        let _ = BufReader::new(stdout).read_line(&mut line);
    }
    if !line.is_empty() {
        print!("{line}");
        return 0;
    }
    let mut error = String::new();
    if let Some(mut stderr) = child.stderr.take() {
        let _ = stderr.read_to_string(&mut error);
    }
    eprint!("{error}");
    child
        .wait()
        .ok()
        .and_then(|status| status.code())
        .unwrap_or(1)
}
