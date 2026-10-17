//! Who is on the shared pages, from the pings the widget sends: the one
//! account of arrivals and departures, for the dashboard and the MCP server.

use std::collections::HashMap;
use std::time::{Duration, Instant};

const LEFT_AFTER: Duration = Duration::from_secs(20);
// A hidden tab's timers wake up once a minute in Chrome, every 40 s in
// Safari: its pings slow down without it leaving.
const HIDDEN_LEFT_AFTER: Duration = Duration::from_secs(90);
const IDLE_AFTER: u64 = 30;
/// Pings are open to anyone with the link: past this many visitors still
/// here, new ones are not tracked, so a flood cannot push the notes out.
const MAX_VISITORS: usize = 200;
/// A page that said goodbye and sent no new ping since has left; a visitor
/// moving to another page says goodbye too, then pings right away.
const GONE_GRACE: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
pub struct Presence {
    pub sid: String,
    pub device: String,
    pub page: String,
    pub visible: bool,
    pub idle: u32,
    pub clicks: u32,
    pub gone: bool,
}

impl Presence {
    /// "active", "tab hidden", "idle 45s" or "left", `away` after its last ping.
    pub fn status(&self, away: Duration) -> String {
        let idle = u64::from(self.idle) + away.as_secs();
        let patience = if self.visible {
            LEFT_AFTER
        } else {
            HIDDEN_LEFT_AFTER
        };
        if self.gone || away > patience {
            "left".into()
        } else if !self.visible {
            "tab hidden".into()
        } else if idle >= IDLE_AFTER {
            format!("idle {idle}s")
        } else {
            "active".into()
        }
    }
}

/// A visitor's last ping, and since when they are here.
pub struct Visitor {
    pub presence: Presence,
    pub seen: Instant,
    pub first: Instant,
}

#[derive(Default)]
pub struct Roster {
    visitors: HashMap<String, Visitor>,
}

impl Roster {
    /// Keeps the visitor's last ping; true when they just arrived.
    pub fn update(&mut self, presence: Presence, now: Instant) -> bool {
        let known = self.visitors.get(&presence.sid);
        if known.is_none() && (presence.gone || self.full(now)) {
            return false;
        }
        let arrived = known.is_none();
        let first = known.map_or(now, |v| v.first);
        let sid = presence.sid.clone();
        let visitor = Visitor {
            presence,
            seen: now,
            first,
        };
        self.visitors.insert(sid, visitor);
        arrived
    }

    fn full(&self, now: Instant) -> bool {
        let here = self
            .visitors
            .values()
            .filter(|v| v.presence.status(now.saturating_duration_since(v.seen)) != "left")
            .count();
        here >= MAX_VISITORS
    }

    /// Forgets the visitors who left, and returns them.
    pub fn departures(&mut self) -> Vec<Presence> {
        let gone: Vec<String> = self
            .visitors
            .iter()
            .filter(|(_, v)| {
                let away = v.seen.elapsed();
                (v.presence.gone && away >= GONE_GRACE) || v.presence.status(away) == "left"
            })
            .map(|(sid, _)| sid.clone())
            .collect();
        gone.iter()
            .filter_map(|sid| self.visitors.remove(sid))
            .map(|v| v.presence)
            .collect()
    }

    /// Everyone known, in the order they arrived.
    pub fn by_arrival(&self) -> Vec<&Visitor> {
        let mut visitors: Vec<&Visitor> = self.visitors.values().collect();
        visitors.sort_by_key(|v| v.first);
        visitors
    }

    pub fn get(&self, sid: &str) -> Option<&Visitor> {
        self.visitors.get(sid)
    }

    pub fn len(&self) -> usize {
        self.visitors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.visitors.is_empty()
    }
}

/// "iPhone · Safari", from the User-Agent and the `Sec-CH-UA` client hint.
pub fn device(agent: &str, hints: &str) -> String {
    let system = [
        ("iPhone", "iPhone"),
        ("iPad", "iPad"),
        ("Android", "Android"),
        ("Macintosh", "Mac"),
        ("Windows", "Windows"),
        ("Linux", "Linux"),
    ]
    .into_iter()
    .find(|(needle, _)| agent.contains(needle))
    .map_or("Unknown", |(_, name)| name);
    // Brave reads as Safari on iOS and as Chrome elsewhere, but names itself
    // at the end of the iOS agent and in the client hint.
    let brave = agent.contains("Brave") || hints.contains("\"Brave\"");
    let browser = [
        ("Edg/", "Edge"),
        ("EdgiOS/", "Edge"),
        ("OPR/", "Opera"),
        ("OPT/", "Opera"),
        ("Firefox/", "Firefox"),
        ("FxiOS/", "Firefox"),
        ("CriOS/", "Chrome"),
        ("Chrome/", "Chrome"),
        ("Safari/", "Safari"),
    ]
    .into_iter()
    .find(|(needle, _)| agent.contains(needle))
    .map_or("browser", |(_, name)| name);
    let browser = if brave { "Brave" } else { browser };
    format!("{system} · {browser}")
}
