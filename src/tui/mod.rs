mod art;
mod audience;
mod boot;
mod cast;
mod dashboard;
mod fx;
pub mod home;
mod hose;
mod keys;
mod scenes;
mod shaders;
mod show;
mod spectacle;
mod traffic;

use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::crossterm::event::{self, Event as Term, KeyEventKind};
use ratatui::style::{Color, Style};
use tokio::sync::watch;

use crate::event::{Event, Hit};
use crate::live::Hub;
use crate::proxy::Replayer;
use audience::Audience;
use boot::Boot;
use keys::Keys;
use show::Show;
use traffic::Traffic;

const BOOT: f32 = 2.2;
const LAUNCH: f32 = 4.0;
const GOODBYE: f32 = 1.6;
const SNIFF: f32 = 0.35;
pub const SLEEP_AFTER: f32 = 60.0;
const MARATHON: f32 = 3600.0;
const FAST_FRAME: Duration = Duration::from_millis(33);
const SLOW_FRAME: Duration = Duration::from_millis(100);
pub struct Theme {
    pub calm: bool,
    pub color: bool,
    pub light: bool,
}

impl Theme {
    pub fn fg(&self, color: Color) -> Style {
        match (self.color, self.light) {
            (false, _) => Style::new(),
            (true, false) => Style::new().fg(color),
            (true, true) => Style::new().fg(fx::for_light(color)),
        }
    }
}

/// True when the terminal reports a light background. Terminals that do not
/// answer get the dark palette, which was the only one before.
pub fn light_terminal() -> bool {
    let reply = crate::os::ask_terminal(b"\x1b]11;?\x07");
    background(&String::from_utf8_lossy(&reply))
        .is_some_and(|(r, g, b)| 0.2126 * r + 0.7152 * g + 0.0722 * b > 0.5)
}

/// Parses `]11;rgb:RRRR/GGGG/BBBB` into channels between 0 and 1.
fn background(reply: &str) -> Option<(f32, f32, f32)> {
    let rgb = &reply[reply.find("]11;rgb:")? + 8..];
    let end = rgb.find(['\x07', '\x1b']).unwrap_or(rgb.len());
    let mut channels = rgb[..end].split('/').map(|hex| {
        let max = 16f32.powi(hex.len() as i32) - 1.0;
        u32::from_str_radix(hex, 16)
            .ok()
            .map(|value| value as f32 / max)
    });
    Some((channels.next()??, channels.next()??, channels.next()??))
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Phase {
    Boot,
    Preflight,
    Digging,
    Launch,
    Dashboard,
    /// The dashboard fading out, between `q` and the goodbye.
    Dissolve,
    Goodbye,
    Failed,
}

pub struct App {
    pub theme: Theme,
    pub phase: Phase,
    pub phase_at: Instant,
    started: Instant,
    pub now: Instant,
    pub dt: f32,
    pub boot: Boot,
    pub traffic: Traffic,
    pub audience: Audience,
    pub show: Show,
    pub keys: Keys,
    exit: Option<i32>,
}

pub fn run(
    rx: Receiver<Event>,
    stop: &watch::Sender<bool>,
    ports: &[u16],
    theme: Theme,
    hub: Arc<Hub>,
    replayer: Replayer,
) -> i32 {
    let mut terminal = ratatui::init();
    let mut app = App::new(ports, theme, hub, replayer);
    let code = loop {
        let now = Instant::now();
        while let Ok(event) = rx.try_recv() {
            app.on_event(event, now);
        }
        app.tick(now);
        if terminal.draw(|frame| app.render(frame)).is_err() {
            break 1;
        }
        if let Some(code) = app.exit {
            break code;
        }
        let budget = if app.busy() { FAST_FRAME } else { SLOW_FRAME };
        if event::poll(budget).unwrap_or(false)
            && let Ok(Term::Key(key)) = event::read()
            && key.kind == KeyEventKind::Press
        {
            app.on_key(key, stop);
        }
    };
    ratatui::restore();
    scenes::recap(&app);
    code
}

impl App {
    fn new(ports: &[u16], theme: Theme, hub: Arc<Hub>, replayer: Replayer) -> Self {
        let calm = theme.calm;
        let now = Instant::now();
        Self {
            theme,
            phase: Phase::Boot,
            phase_at: now,
            started: now,
            now,
            dt: 0.0,
            boot: Boot::new(ports),
            traffic: Traffic::new(replayer, now),
            audience: Audience::new(hub),
            show: Show::new(calm),
            keys: Keys::default(),
            exit: None,
        }
    }

    pub fn t(&self) -> f32 {
        (self.now - self.phase_at).as_secs_f32()
    }

    /// Seconds since the app started, for animations that span scenes.
    pub fn elapsed(&self) -> f32 {
        (self.now - self.started).as_secs_f32()
    }

    fn go(&mut self, phase: Phase) {
        self.phase = phase;
        self.phase_at = self.now;
        if phase == Phase::Dashboard {
            self.show.shaders.entry();
        }
    }

    /// Animations need 30 fps; a quiet dashboard is fine at 10.
    fn busy(&self) -> bool {
        !self.theme.calm && (self.phase != Phase::Dashboard || self.show.busy())
    }

    pub fn live_for(&self) -> u64 {
        self.boot.ready_at.map_or(0, |at| (self.now - at).as_secs())
    }

    pub fn sleeping(&self) -> bool {
        let since = self
            .traffic
            .last_hit
            .or(self.boot.ready_at)
            .map_or(0.0, |at| (self.now - at).as_secs_f32());
        since > SLEEP_AFTER
    }

    pub fn toast(&mut self, title: impl Into<String>, body: impl Into<String>, gold: bool) {
        self.show.toast(self.now, title.into(), body.into(), gold);
    }

    fn unlock(&mut self, key: &str) {
        self.show.unlock(self.now, key);
    }

    fn on_event(&mut self, event: Event, now: Instant) {
        self.now = now;
        match event {
            Event::PortChecked { .. }
            | Event::FetchingCloudflared
            | Event::TunnelStarting
            | Event::TunnelUrl(_)
            | Event::TunnelRegistered
            | Event::DnsAttempt(_) => self.boot.on(event, now),
            Event::Ready { .. } => {
                self.boot.on(event, now);
                if local_hour() < 5 {
                    self.unlock("owl");
                }
            }
            Event::Request(hit) => self.on_hit(hit),
            Event::Presence(presence) => {
                if self.audience.presence(presence, now) {
                    self.show.shaders.arrival();
                }
            }
            Event::Feedback(note) => {
                let (title, excerpt) = self.audience.note(note);
                self.toast(title, excerpt, false);
                self.unlock("critic");
            }
            Event::Chat(said) => {
                let (title, excerpt) = self.audience.said(said);
                self.toast(title, excerpt, false);
            }
            Event::Pointer(pointer) => self.audience.pointer(pointer, now),
            // For the agent; the dashboard shows the traffic instead.
            Event::PageError(_) => {}
            Event::Reacted(reaction) => {
                let (title, label) = self.audience.reacted(&reaction);
                self.toast(title, label, false);
                self.show.celebrate(now, reaction.kind);
            }
            Event::Replayed { n, status, ms } => {
                if let Some((title, body)) = self.traffic.replayed(n, status, ms) {
                    self.toast(title, body, false);
                }
            }
            Event::PortHealth { port, ok } => {
                self.boot.health(port, ok);
                if ok {
                    self.toast(
                        format!("♥ port {port} is back"),
                        "The bunny is relieved.",
                        false,
                    );
                } else {
                    self.toast(
                        format!("✖ port {port} went quiet"),
                        "Visitors see the bunny page.",
                        false,
                    );
                }
            }
            Event::Done(Ok(())) => {
                if self.keys.quitting.is_none() {
                    self.exit = Some(0);
                }
            }
            Event::Done(Err(failure)) => {
                if self.keys.quitting.is_none() {
                    self.boot.failure = Some(failure);
                    self.go(Phase::Failed);
                }
            }
        }
    }

    fn on_hit(&mut self, hit: Hit) {
        self.boot.hit(hit.port);
        if hit.status >= 500 {
            self.show.shaders.crash();
        }
        self.show.sprite(self.now, &hit);
        for key in self.traffic.record(hit, self.now) {
            self.unlock(key);
        }
    }

    fn tick(&mut self, now: Instant) {
        self.dt = (now - self.now).as_secs_f32().min(0.1);
        self.now = now;
        let t = self.t();
        let calm = self.theme.calm;
        match self.phase {
            Phase::Boot if calm || t >= BOOT => self.go(Phase::Preflight),
            Phase::Preflight => {
                let checked = self.boot.ports.iter().all(|p| p.checked.is_some());
                let shown = calm || t >= 0.3 + SNIFF * self.boot.ports.len() as f32 + 0.4;
                if checked && shown {
                    self.go(Phase::Digging);
                }
            }
            // The dragon flees before LIVE!.
            Phase::Digging
                if self
                    .boot
                    .ready_at
                    .is_some_and(|at| calm || (now - at).as_secs_f32() >= spectacle::FLEE) =>
            {
                self.go(if calm {
                    Phase::Dashboard
                } else {
                    Phase::Launch
                })
            }
            Phase::Launch if t >= LAUNCH => self.go(Phase::Dashboard),
            Phase::Dissolve if t * 1000.0 >= shaders::DISSOLVE_MS as f32 => self.go(Phase::Goodbye),
            Phase::Goodbye if calm || t >= GOODBYE => self.exit = Some(0),
            _ => {}
        }
        if self
            .boot
            .ready_at
            .is_some_and(|at| (now - at).as_secs_f32() >= MARATHON)
        {
            self.unlock("marathon");
        }

        self.boot.tick(self.dt);
        self.traffic.tick(now);
        self.show.tick(now, self.dt);
    }

    fn render(&mut self, frame: &mut Frame) {
        self.show.keep_clear = None;
        self.show.spots = shaders::Spots::default();
        match self.phase {
            Phase::Boot => scenes::boot(self, frame),
            Phase::Preflight => scenes::preflight(self, frame),
            Phase::Digging => scenes::digging(self, frame),
            Phase::Launch => scenes::launch(self, frame),
            Phase::Dashboard | Phase::Dissolve => dashboard::render(self, frame),
            Phase::Goodbye => scenes::goodbye(self, frame),
            Phase::Failed => scenes::failed(self, frame),
        }
        let area = frame.area();
        self.show
            .shaders
            .render(frame.buffer_mut(), area, &self.show.spots, self.dt);
        if !self.theme.calm {
            scenes::fun(self, frame);
            self.show
                .particles
                .draw(frame.buffer_mut(), &self.theme, self.show.keep_clear);
        }
        dashboard::overlays(self, frame);
    }
}

pub fn plural(count: usize, word: &str) -> String {
    format!("{count} {word}{}", if count == 1 { "" } else { "s" })
}

/// The next item up or down from `current`, entering at either end.
fn stepped<T: PartialEq + Clone>(items: &[T], current: Option<&T>, down: bool) -> Option<T> {
    let last = items.len().checked_sub(1)?;
    let at = current.and_then(|c| items.iter().position(|item| item == c));
    let next = match (at, down) {
        (None, true) => 0,
        (None, false) => last,
        (Some(i), true) => (i + 1).min(last),
        (Some(i), false) => i.saturating_sub(1),
    };
    Some(items[next].clone())
}

fn local_hour() -> u32 {
    chrono::Timelike::hour(&chrono::Local::now())
}
