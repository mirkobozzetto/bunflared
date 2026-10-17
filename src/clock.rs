//! Times as bunflared writes them: in file names, in the logs, for how long.

/// A file name that sorts in time order, to the millisecond.
pub fn stamp() -> String {
    chrono::Local::now()
        .format("%Y-%m-%d_%H-%M-%S%.3f")
        .to_string()
}

/// The time of day, as the logs show it.
pub fn clock() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

pub fn uptime(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs}s"),
        60..3600 => format!("{}m {:02}s", secs / 60, secs % 60),
        _ => format!("{}h {:02}m", secs / 3600, secs % 3600 / 60),
    }
}
