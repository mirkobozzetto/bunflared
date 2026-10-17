use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};

use crate::clock;
use crate::curl;
use crate::event::Hit;
use crate::proxy::Replayer;

const LOG_KEEP: usize = 200;
const SPARK_KEEP: usize = 120;

pub struct Row {
    pub hit: Hit,
    pub clock: String,
    /// Its rank since the start, a key that survives new rows.
    pub n: u32,
    pub replayed: Option<Replay>,
}

pub enum Replay {
    Pending,
    Done { status: u16, ms: u32 },
    Failed,
}

/// The requests: their log, the counters, the sparkline, the one picked.
pub struct Traffic {
    pub rows: VecDeque<Row>,
    pub total: u32,
    pub classes: [u32; 4],
    pub visitors: HashSet<String>,
    pub sockets: u32,
    pub ms_sum: u64,
    pub per_second: VecDeque<u64>,
    bucket_at: Instant,
    pub last_hit: Option<Instant>,
    pub last_error: Option<Instant>,
    /// The request picked in the log, by rank.
    pub picked: Option<u32>,
    /// The request whose details are open, by rank.
    pub details: Option<u32>,
    pub details_scroll: u16,
    /// What the shown rows contain in `method status path`, any case.
    pub filter: String,
    replayer: Replayer,
}

/// 2xx, 3xx, 4xx or 5xx, as 0 to 3.
pub fn class(status: u16) -> usize {
    match status {
        0..300 => 0,
        300..400 => 1,
        400..500 => 2,
        _ => 3,
    }
}

impl Traffic {
    pub fn new(replayer: Replayer, now: Instant) -> Self {
        Self {
            rows: VecDeque::new(),
            total: 0,
            classes: [0; 4],
            visitors: HashSet::new(),
            sockets: 0,
            ms_sum: 0,
            per_second: VecDeque::from(vec![0; SPARK_KEEP]),
            bucket_at: now,
            last_hit: None,
            last_error: None,
            picked: None,
            details: None,
            details_scroll: 0,
            filter: String::new(),
            replayer,
        }
    }

    /// Counts and logs a request; returns the achievements it reaches.
    pub fn record(&mut self, hit: Hit, now: Instant) -> Vec<&'static str> {
        let mut reached = Vec::new();
        self.total += 1;
        self.classes[class(hit.status)] += 1;
        self.ms_sum += hit.ms as u64;
        if let Some(back) = self.per_second.back_mut() {
            *back += 1;
        }
        if self.visitors.insert(hit.visitor.clone()) {
            match self.visitors.len() {
                1 => reached.push("first"),
                5 => reached.push("party"),
                _ => {}
            }
        }
        match self.total {
            100 => reached.push("centurion"),
            1000 => reached.push("thousand"),
            _ => {}
        }
        if hit.ms < 10 {
            reached.push("speed");
        }
        if hit.upgrade && hit.status == 101 {
            self.sockets += 1;
            reached.push("tube");
        }
        if hit.status >= 500 {
            self.last_error = Some(now);
            reached.push("survivor");
        }
        self.last_hit = Some(now);
        self.rows.push_front(Row {
            hit,
            clock: clock::clock(),
            n: self.total,
            replayed: None,
        });
        self.rows.truncate(LOG_KEEP);
        reached
    }

    /// The rows the filter lets through, newest first.
    pub fn shown(&self) -> impl Iterator<Item = &Row> {
        let filter = self.filter.to_lowercase();
        self.rows.iter().filter(move |row| {
            let hit = &row.hit;
            format!("{} {} {}", hit.method, hit.status, hit.path)
                .to_lowercase()
                .contains(&filter)
        })
    }

    /// A bar of the sparkline per second.
    pub fn tick(&mut self, now: Instant) {
        while now.duration_since(self.bucket_at) >= Duration::from_secs(1) {
            self.bucket_at += Duration::from_secs(1);
            self.per_second.push_back(0);
            if self.per_second.len() > SPARK_KEEP {
                self.per_second.pop_front();
            }
        }
    }

    /// Sends a request of the log again, unless it cannot be sent as it was.
    pub fn replay(&mut self, n: u32) -> Result<(), String> {
        let Some(row) = self.rows.iter_mut().find(|row| row.n == n) else {
            return Ok(());
        };
        if let Some(reason) = row.hit.unreplayable() {
            return Err(reason);
        }
        self.replayer.replay(n, &row.hit);
        row.replayed = Some(Replay::Pending);
        Ok(())
    }

    /// A request of the log as a curl command, or why there is none.
    pub fn curl(&self, n: u32) -> Option<Result<String, String>> {
        let row = self.rows.iter().find(|row| row.n == n)?;
        Some(curl::curl(&row.hit))
    }

    /// Notes how a replay went; returns the toast that tells it.
    pub fn replayed(
        &mut self,
        n: u32,
        status: Result<u16, String>,
        ms: u32,
    ) -> Option<(String, String)> {
        let row = self.rows.iter_mut().find(|row| row.n == n)?;
        let title = format!("↻ {} {}", row.hit.method, row.hit.path);
        let body = match status {
            Ok(status) => {
                row.replayed = Some(Replay::Done { status, ms });
                format!("{} before, {status} now, in {ms} ms.", row.hit.status)
            }
            Err(err) => {
                row.replayed = Some(Replay::Failed);
                format!("Failed: {err}")
            }
        };
        Some((title, body))
    }
}
