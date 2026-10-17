//! Times as bunflared writes them: in file names, in the logs, for how long.

use std::time::Duration;

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

/// A duration as `--for` takes it: `90s`, `30m`, `2h`, `1h30m`.
pub fn duration(text: &str) -> Result<Duration, String> {
    let bad = || format!("{text:?} is not a duration: write 90s, 30m, 2h or 1h30m");
    let (mut secs, mut digits) = (0u64, String::new());
    for c in text.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        let unit = match c {
            's' => 1,
            'm' => 60,
            'h' => 3600,
            _ => return Err(bad()),
        };
        let n: u64 = digits.parse().map_err(|_| bad())?;
        secs = n
            .checked_mul(unit)
            .and_then(|n| secs.checked_add(n))
            .ok_or_else(bad)?;
        digits.clear();
    }
    if !digits.is_empty() || secs == 0 {
        return Err(bad());
    }
    Ok(Duration::from_secs(secs))
}

pub fn uptime(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs}s"),
        60..3600 => format!("{}m {:02}s", secs / 60, secs % 60),
        _ => format!("{}h {:02}m", secs / 3600, secs % 3600 / 60),
    }
}
