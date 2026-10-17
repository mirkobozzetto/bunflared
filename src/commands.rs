//! The commands that act on shares already running: `ls`, `down`, and
//! `--detach`, which starts one and leaves it running.

use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::clock::uptime;
use crate::os;
use crate::state::{self, Record};

const DOWN_GRACE: Duration = Duration::from_secs(3);

pub fn list(json: bool) -> i32 {
    let records = state::live();
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
    println!(
        "{:<8} {:<64} {:<16} {:<10} LEFT",
        "ID", "URL", "PORTS", "UPTIME"
    );
    for record in records {
        let ports: Vec<String> = record.routes.values().map(u16::to_string).collect();
        let left = record
            .expires_at
            .map_or("-".into(), |at| uptime(at.saturating_sub(now)));
        println!(
            "{:<8} {:<64} {:<16} {:<10} {left}",
            record.id,
            record.url,
            ports.join(","),
            uptime(now.saturating_sub(record.started_at))
        );
    }
    0
}

pub fn down(id: Option<String>, all: bool) -> i32 {
    let targets: Vec<Record> = state::live()
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
        if record.hosted() {
            if let Some(path) = state::file(&record.id) {
                let _ = fs::remove_file(path);
            }
        } else {
            os::terminate(record.pid);
        }
    }
    // A hosted share is gone once its tunnel is.
    let running = |record: &Record| {
        if record.hosted() {
            record.tunnel_pid != 0 && os::alive(record.tunnel_pid)
        } else {
            os::alive(record.pid)
        }
    };
    let deadline = Instant::now() + DOWN_GRACE;
    while Instant::now() < deadline && targets.iter().any(running) {
        std::thread::sleep(Duration::from_millis(50));
    }
    for record in &targets {
        if running(record) {
            if !record.hosted() {
                os::kill(record.pid);
            }
            os::kill(record.tunnel_pid);
        }
        if let Some(path) = state::file(&record.id) {
            let _ = fs::remove_file(path);
        }
        println!("Stopped {} {}", record.id, record.url);
    }
    0
}

/// Starts the share in its own session and returns once it prints its ready
/// line, so the caller's shell is free while the share keeps running.
pub fn detach(
    ports: &[u16],
    no_widget: bool,
    local: bool,
    expires_in: Option<Duration>,
    password: Option<&str>,
) -> i32 {
    let Ok(exe) = std::env::current_exe() else {
        eprintln!(r#"{{"error":"cannot find the bunflared executable","code":1}}"#);
        return 1;
    };
    let mut command = Command::new(exe);
    command
        .args(ports.iter().map(u16::to_string))
        .arg("--json")
        .args(no_widget.then_some("--no-widget"))
        .args(local.then_some("--local"))
        .args(expires_in.map(|left| format!("--for={}s", left.as_secs())))
        // ponytail: on the command line, so `ps` shows it to this computer's users.
        .args(password.map(|password| format!("--password={password}")))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    os::detach(&mut command);
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
