//! What differs between Unix and Windows: process control, signals, folders.

use std::path::PathBuf;

pub fn home() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).map(PathBuf::from)
}

/// Where bunflared keeps its share records and its own copy of cloudflared.
pub fn data_dir() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| home().map(|home| home.join(".local/state")))
    };
    Some(base?.join("bunflared"))
}

#[cfg(unix)]
mod imp {
    use std::os::unix::process::CommandExt;

    pub fn alive(pid: u32) -> bool {
        pid != 0 && unsafe { libc::kill(pid as i32, 0) } == 0
    }

    /// Asks the share to stop, so it cleans up after itself.
    pub fn terminate(pid: u32) {
        unsafe { libc::kill(pid as i32, libc::SIGTERM) };
    }

    pub fn kill(pid: u32) {
        if pid != 0 {
            unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        }
    }

    /// Its own session: closing the caller's terminal does not reach it.
    pub fn detach(command: &mut std::process::Command) {
        unsafe {
            command.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }

    pub fn no_window(_: &mut tokio::process::Command) {}

    /// Waits for Ctrl-C, SIGTERM or SIGHUP; true when the terminal hung up.
    pub async fn quit() -> bool {
        use tokio::signal::unix::{SignalKind, signal};
        let handler = |kind| signal(kind).expect("signal handler");
        let mut interrupt = handler(SignalKind::interrupt());
        let mut terminate = handler(SignalKind::terminate());
        let mut hangup = handler(SignalKind::hangup());
        tokio::select! {
            _ = interrupt.recv() => false,
            _ = terminate.recv() => false,
            _ = hangup.recv() => true,
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub fn alive(pid: u32) -> bool {
        let filter = format!("PID eq {pid}");
        Command::new("tasklist")
            .args(["/FI", &filter, "/NH", "/FO", "CSV"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .is_ok_and(|out| String::from_utf8_lossy(&out.stdout).contains(&format!("\"{pid}\"")))
    }

    // ponytail: a console process has no polite stop on Windows; the forced
    // kill takes cloudflared with it (/T) and `ls` drops the stale record.
    pub fn terminate(pid: u32) {
        kill(pid);
    }

    pub fn kill(pid: u32) {
        if pid != 0 {
            let _ = Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        }
    }

    pub fn detach(command: &mut Command) {
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }

    /// A detached share has no console; cloudflared must not pop one open.
    pub fn no_window(command: &mut tokio::process::Command) {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    /// Waits for Ctrl-C, Ctrl-Break or the console closing; true for the latter.
    pub async fn quit() -> bool {
        use tokio::signal::windows::{ctrl_break, ctrl_c, ctrl_close};
        let mut interrupt = ctrl_c().expect("ctrl-c handler");
        let mut brk = ctrl_break().expect("ctrl-break handler");
        let mut close = ctrl_close().expect("ctrl-close handler");
        tokio::select! {
            _ = interrupt.recv() => false,
            _ = brk.recv() => false,
            _ = close.recv() => true,
        }
    }
}

pub use imp::*;
