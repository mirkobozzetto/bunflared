//! The TCP ports listening on this machine, and the apps behind them, read
//! from what each system already ships: lsof and ps, /proc, netstat.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Below this, ports belong to the system.
const FIRST_PORT: u16 = 1024;
const OURS: [&str; 3] = ["bunf", "bunflared", "cloudflared"];
/// Daemons run from the root folder, packaged services (Homebrew's postgres,
/// a distribution's redis) from these.
const SERVICE_FOLDERS: [&str; 5] = ["/", "/opt/homebrew", "/usr", "/var", "/Library"];

#[derive(Clone, Debug, PartialEq)]
pub struct Listener {
    pub port: u16,
    pub pid: u32,
    pub program: String,
    /// Where it was started, when the system says.
    pub folder: Option<PathBuf>,
    /// Seconds it has been running, when the system says.
    pub age: Option<u64>,
}

/// The apps listening here, one per port: not the system's, not ours.
pub fn scan() -> Vec<Listener> {
    let me = std::process::id();
    let mut found: Vec<Listener> = imp::listeners()
        .into_iter()
        .filter(|l| l.port >= FIRST_PORT && l.pid != me)
        .filter(|l| !OURS.contains(&l.program.trim_end_matches(".exe")))
        .filter(|l| !l.folder.as_deref().is_some_and(service))
        .collect();
    found.sort_by_key(|l| l.port);
    found.dedup_by_key(|l| l.port);
    #[cfg(unix)]
    ages(&mut found);
    found
}

fn service(folder: &Path) -> bool {
    SERVICE_FOLDERS.iter().any(|&root| {
        if root == "/" {
            folder == Path::new(root)
        } else {
            folder.starts_with(root)
        }
    })
}

fn output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default()
}

#[cfg(unix)]
fn ages(found: &mut [Listener]) {
    let pids: Vec<String> = found.iter().map(|l| l.pid.to_string()).collect();
    if pids.is_empty() {
        return;
    }
    let text = output("ps", &["-o", "pid=,etime=", "-p", &pids.join(",")]);
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let pid = fields.next().and_then(|pid| pid.parse::<u32>().ok());
        let (Some(pid), Some(age)) = (pid, fields.next().and_then(elapsed)) else {
            continue;
        };
        for listener in found.iter_mut().filter(|l| l.pid == pid) {
            listener.age = Some(age);
        }
    }
}

/// `[[dd-]hh:]mm:ss`, as ps prints the time since a process started.
#[cfg(unix)]
fn elapsed(text: &str) -> Option<u64> {
    let (days, clock) = text.split_once('-').unwrap_or(("0", text));
    let clock = clock.split(':').try_fold(0, |secs: u64, part| {
        Some(secs * 60 + part.parse::<u64>().ok()?)
    })?;
    Some(days.parse::<u64>().ok()? * 86_400 + clock)
}

#[cfg(all(unix, not(target_os = "linux")))]
mod imp {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use super::{Listener, output};

    pub fn listeners() -> Vec<Listener> {
        // -F: one field per line, tagged p (pid), c (command), n (address);
        // +c 0: the whole command name, not its first nine bytes.
        let text = output(
            "lsof",
            &["+c", "0", "-nP", "-iTCP", "-sTCP:LISTEN", "-Fpcn"],
        );
        let (mut pid, mut program) = (0, String::new());
        let mut found = Vec::new();
        for line in text.lines().filter(|line| !line.is_empty()) {
            let (tag, value) = line.split_at(1);
            match tag {
                "p" => pid = value.parse().unwrap_or(0),
                "c" => program = value.to_string(),
                "n" => {
                    if let Some(port) = value.rsplit(':').next().and_then(|p| p.parse().ok()) {
                        found.push(Listener {
                            port,
                            pid,
                            program: program.clone(),
                            folder: None,
                            age: None,
                        });
                    }
                }
                _ => {}
            }
        }
        let folders = folders(&found);
        for listener in &mut found {
            listener.folder = folders.get(&listener.pid).cloned();
        }
        found
    }

    fn folders(found: &[Listener]) -> HashMap<u32, PathBuf> {
        let mut pids: Vec<String> = found.iter().map(|l| l.pid.to_string()).collect();
        pids.dedup();
        if pids.is_empty() {
            return HashMap::new();
        }
        let text = output("lsof", &["-a", "-d", "cwd", "-Fn", "-p", &pids.join(",")]);
        let mut pid = 0;
        let mut folders = HashMap::new();
        for line in text.lines().filter(|line| !line.is_empty()) {
            match line.split_at(1) {
                ("p", value) => pid = value.parse().unwrap_or(0),
                ("n", value) => {
                    folders.insert(pid, PathBuf::from(value));
                }
                _ => {}
            }
        }
        folders
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use std::collections::HashMap;
    use std::fs;

    use super::Listener;

    const LISTEN: &str = "0A";

    pub fn listeners() -> Vec<Listener> {
        let mut sockets = HashMap::new();
        for table in ["/proc/net/tcp", "/proc/net/tcp6"] {
            let text = fs::read_to_string(table).unwrap_or_default();
            for line in text.lines().skip(1) {
                let fields: Vec<&str> = line.split_whitespace().collect();
                let port = fields
                    .get(1)
                    .and_then(|local| local.rsplit(':').next())
                    .and_then(|hex| u16::from_str_radix(hex, 16).ok());
                if let (Some(port), Some(&LISTEN), Some(inode)) =
                    (port, fields.get(3), fields.get(9))
                {
                    sockets.insert(format!("socket:[{inode}]"), port);
                }
            }
        }
        if sockets.is_empty() {
            return Vec::new();
        }
        // Only our own processes open up: a socket left unplaced belongs to
        // another user or to the system.
        let mut found = Vec::new();
        for entry in fs::read_dir("/proc").into_iter().flatten().flatten() {
            let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
                continue;
            };
            let Ok(fds) = fs::read_dir(entry.path().join("fd")) else {
                continue;
            };
            for fd in fds.flatten() {
                let Some(&port) = fs::read_link(fd.path())
                    .ok()
                    .and_then(|target| sockets.get(target.to_string_lossy().as_ref()))
                else {
                    continue;
                };
                found.push(Listener {
                    port,
                    pid,
                    program: fs::read_to_string(entry.path().join("comm"))
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                    folder: fs::read_link(entry.path().join("cwd")).ok(),
                    age: None,
                });
            }
        }
        found
    }
}

#[cfg(windows)]
mod imp {
    use std::collections::HashMap;

    use super::{Listener, output};

    /// tasklist's session number for the services.
    const SERVICES: &str = "0";

    pub fn listeners() -> Vec<Listener> {
        // "Image Name","PID","Session Name","Session#","Mem Usage"
        let programs: HashMap<u32, String> = output("tasklist", &["/FO", "CSV", "/NH"])
            .lines()
            .filter_map(|line| {
                let fields: Vec<&str> = line.trim().trim_matches('"').split("\",\"").collect();
                let session = fields.get(3)?;
                (*session != SERVICES).then_some(())?;
                Some((fields.get(1)?.parse().ok()?, fields.first()?.to_string()))
            })
            .collect();
        // Proto, local address, foreign address, state, pid. The state is
        // translated on some systems; a listening socket's foreign port is 0.
        output("netstat", &["-ano"])
            .lines()
            .filter_map(|line| {
                let fields: Vec<&str> = line.split_whitespace().collect();
                let [proto, local, foreign, _, pid] = fields[..] else {
                    return None;
                };
                if !proto.starts_with("TCP") || !foreign.ends_with(":0") {
                    return None;
                }
                let pid = pid.parse().ok()?;
                Some(Listener {
                    port: local.rsplit(':').next()?.parse().ok()?,
                    pid,
                    program: programs.get(&pid)?.clone(),
                    folder: None,
                    age: None,
                })
            })
            .collect()
    }
}
