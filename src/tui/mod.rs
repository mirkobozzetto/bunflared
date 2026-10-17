mod art;
mod audience;
mod boot;
mod cast;
mod dashboard;
mod fx;
pub mod home;
mod hose;
mod scenes;
mod shaders;
mod show;
mod spectacle;
mod traffic;

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::crossterm::event::{
    self, Event as Term, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use ratatui::style::{Color, Style};
use tokio::sync::watch;

use crate::clipboard;
use crate::clock::uptime;
use crate::event::{Event, Hit};
use crate::live::{self, Command, Hub, REACTIONS};
use crate::notes;
use crate::proxy::Replayer;
use audience::Audience;
use boot::Boot;
use show::Show;
use traffic::Traffic;

const BOOT: f32 = 2.2;
const LAUNCH: f32 = 4.0;
const GOODBYE: f32 = 1.6;
const SNIFF: f32 = 0.35;
pub const SLEEP_AFTER: f32 = 60.0;
const MARATHON: f32 = 3600.0;
const STAMPEDE_PRESSES: usize = 5;
const STAMPEDE_WINDOW: Duration = Duration::from_millis(1500);
// No command key in it, or typing it would copy, open or quit.
const CARROT_WORD: &str = "yum";
const FAST_FRAME: Duration = Duration::from_millis(33);
const SLOW_FRAME: Duration = Duration::from_millis(100);
const KONAMI: [KeyCode; 10] = [
    KeyCode::Up,
    KeyCode::Up,
    KeyCode::Down,
    KeyCode::Down,
    KeyCode::Left,
    KeyCode::Right,
    KeyCode::Left,
    KeyCode::Right,
    KeyCode::Char('b'),
    KeyCode::Char('a'),
];

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

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Focus {
    Visitors,
    Log,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Ask {
    Go,
    Chat,
    React,
}

/// A text box open over the dashboard: while it is, keys are text.
pub struct Prompt {
    pub ask: Ask,
    pub text: String,
    pub pick: Option<usize>,
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
    pub focus: Option<Focus>,
    pub prompt: Option<Prompt>,
    pub qr: bool,
    pub help: bool,
    keys: VecDeque<KeyCode>,
    typed: String,
    b_presses: VecDeque<Instant>,
    pub quitting: Option<Instant>,
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
    app.farewell();
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
            focus: None,
            prompt: None,
            qr: false,
            help: false,
            keys: VecDeque::new(),
            typed: String::new(),
            b_presses: VecDeque::new(),
            quitting: None,
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
                if self.quitting.is_none() {
                    self.exit = Some(0);
                }
            }
            Event::Done(Err(failure)) => {
                if self.quitting.is_none() {
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

    fn on_key(&mut self, key: KeyEvent, stop: &watch::Sender<bool>) {
        let ctrl_c =
            key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
        if self.prompt.is_some() && self.phase == Phase::Dashboard && !ctrl_c {
            return self.on_prompt_key(key);
        }
        let quit = ctrl_c || key.code == KeyCode::Char('q');
        match self.phase {
            Phase::Goodbye | Phase::Dissolve => {
                if quit {
                    self.exit = Some(0);
                }
                return;
            }
            Phase::Failed => {
                self.exit = Some(self.boot.failure.as_ref().map_or(1, |f| f.code));
                return;
            }
            _ if quit => {
                self.quitting = Some(self.now);
                self.help = false;
                self.qr = false;
                self.prompt = None;
                self.traffic.details = None;
                let _ = stop.send(true);
                let leaving = if self.phase == Phase::Dashboard && !self.theme.calm {
                    self.show.shaders.dissolve();
                    Phase::Dissolve
                } else {
                    Phase::Goodbye
                };
                self.go(leaving);
                return;
            }
            Phase::Boot => return self.go(Phase::Preflight),
            Phase::Launch => return self.go(Phase::Dashboard),
            _ => {}
        }

        self.keys.push_back(key.code);
        if self.keys.len() > KONAMI.len() {
            self.keys.pop_front();
        }
        if self.keys.iter().eq(KONAMI.iter()) {
            self.show.disco = !self.show.disco && !self.theme.calm;
            self.show.shaders.disco(self.show.disco);
            self.keys.clear();
            self.unlock("disco");
        }
        if let KeyCode::Char(ch) = key.code {
            self.typed.push(ch);
            if self.typed.len() > 16 {
                self.typed.remove(0);
            }
            if self.typed.ends_with(CARROT_WORD) {
                self.show.carrots_until = Some(self.now + Duration::from_secs(5));
                self.typed.clear();
                self.unlock("carrot");
            }
        }

        let dashboard = self.phase == Phase::Dashboard;
        if let (Some(n), true) = (self.traffic.details, dashboard) {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => self.traffic.details = None,
                KeyCode::Up => {
                    self.traffic.details_scroll = self.traffic.details_scroll.saturating_sub(1)
                }
                KeyCode::Down => {
                    self.traffic.details_scroll = self.traffic.details_scroll.saturating_add(1)
                }
                KeyCode::PageUp => {
                    self.traffic.details_scroll = self.traffic.details_scroll.saturating_sub(10)
                }
                KeyCode::PageDown => {
                    self.traffic.details_scroll = self.traffic.details_scroll.saturating_add(10)
                }
                KeyCode::Char('p') => self.replay(n),
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Esc if self.help || self.qr => {
                self.help = false;
                self.qr = false;
            }
            KeyCode::Esc => {
                self.focus = None;
                self.audience.select(None);
                self.traffic.picked = None;
            }
            KeyCode::Tab if dashboard => {
                self.focus = Some(match self.focus {
                    Some(Focus::Visitors) => Focus::Log,
                    _ => Focus::Visitors,
                });
            }
            KeyCode::Up | KeyCode::Down if dashboard => self.step(key.code == KeyCode::Down),
            KeyCode::Char('g') if dashboard => {
                self.prompt = Some(Prompt {
                    ask: Ask::Go,
                    text: String::new(),
                    pick: None,
                });
            }
            KeyCode::Char('m') if dashboard => {
                self.prompt = Some(Prompt {
                    ask: Ask::Chat,
                    text: String::new(),
                    pick: None,
                });
            }
            KeyCode::Enter if dashboard && self.focus == Some(Focus::Log) => {
                if self.traffic.picked.is_some() {
                    self.traffic.details = self.traffic.picked;
                    self.traffic.details_scroll = 0;
                }
            }
            KeyCode::Char('p') if dashboard => match self.traffic.picked {
                Some(n) => self.replay(n),
                None => self.toast("Pick a request first", "Tab, then ↑ ↓.", false),
            },
            KeyCode::Char('e') if dashboard => {
                self.prompt = Some(Prompt {
                    ask: Ask::React,
                    text: String::new(),
                    pick: Some(0),
                });
            }
            KeyCode::Char('x') if dashboard => self.audience.chat.clear(),
            KeyCode::Char('R') if dashboard => {
                let reached = self.audience.send(&Command::Reload);
                self.sent("↻ Reload", reached);
            }
            KeyCode::Char('?') => self.help = !self.help,
            // The code already on screen flashes instead of opening twice.
            KeyCode::Char('r') if self.boot.qr_code.is_some() => {
                if self.qr || self.show.spots.qr.is_none() {
                    self.qr = !self.qr;
                } else if self.theme.calm {
                    self.toast("The code is on the right", "Scan it with a phone.", false);
                } else {
                    self.show.shaders.flash_qr();
                }
            }
            KeyCode::Char('c') => {
                if let Some(url) = &self.boot.url {
                    self.boot.copied = clipboard::copy(url);
                    let body = if self.boot.copied {
                        "Paste it anywhere."
                    } else {
                        "No clipboard tool found."
                    };
                    self.toast("Link copied", body, false);
                }
            }
            KeyCode::Char('o') => {
                if let Some(url) = self
                    .boot
                    .url
                    .clone()
                    .filter(|_| self.boot.ready_at.is_some())
                {
                    clipboard::open(&url);
                    self.toast("Opening your browser", "Say hi to the bunny.", false);
                }
            }
            KeyCode::Char('f') if !self.theme.calm => {
                if self.show.firework() {
                    self.unlock("pyro");
                }
            }
            KeyCode::Char('b') if !self.theme.calm => {
                self.b_presses.push_back(self.now);
                while self
                    .b_presses
                    .front()
                    .is_some_and(|at| self.now - *at > STAMPEDE_WINDOW)
                {
                    self.b_presses.pop_front();
                }
                if self.b_presses.len() >= STAMPEDE_PRESSES {
                    self.b_presses.clear();
                    self.show.stampede();
                    self.unlock("stampede");
                }
            }
            _ => {}
        }
    }

    /// Moves the selection in the focused panel, the visitors by default.
    fn step(&mut self, down: bool) {
        match *self.focus.get_or_insert(Focus::Visitors) {
            Focus::Visitors => {
                let sids: Vec<String> = dashboard::roster(self)
                    .iter()
                    .map(|v| v.presence.sid.clone())
                    .collect();
                let next = stepped(&sids, self.audience.selected.as_ref(), down);
                self.audience.select(next);
            }
            Focus::Log => {
                let ranks: Vec<u32> = self.traffic.rows.iter().map(|row| row.n).collect();
                self.traffic.picked = stepped(&ranks, self.traffic.picked.as_ref(), down);
            }
        }
    }

    fn on_prompt_key(&mut self, key: KeyEvent) {
        let count = self
            .prompt
            .as_ref()
            .map_or(0, |prompt| self.audience.suggestions(prompt).len());
        let Some(prompt) = self.prompt.as_mut() else {
            return;
        };
        let digit = match key.code {
            KeyCode::Char(ch) => ch.to_digit(10).map(|d| d as usize),
            _ => None,
        };
        if prompt.ask == Ask::React {
            if let Some(n) = digit.filter(|n| (1..=REACTIONS.len()).contains(n)) {
                prompt.pick = Some(n - 1);
                if let Some(prompt) = self.prompt.take() {
                    self.submit(prompt);
                }
            } else if let KeyCode::Char(_) = key.code {
                return;
            }
        }
        let Some(prompt) = self.prompt.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc => self.prompt = None,
            KeyCode::Enter => {
                if let Some(prompt) = self.prompt.take() {
                    self.submit(prompt);
                }
            }
            KeyCode::Backspace => {
                prompt.text.pop();
                prompt.pick = None;
            }
            KeyCode::Tab | KeyCode::Down if count > 0 => {
                prompt.pick = Some(prompt.pick.map_or(0, |i| (i + 1) % count));
            }
            KeyCode::BackTab | KeyCode::Up if count > 0 => {
                prompt.pick = Some(prompt.pick.map_or(count - 1, |i| (i + count - 1) % count));
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                prompt.text.push(ch);
                prompt.pick = None;
            }
            _ => {}
        }
    }

    fn submit(&mut self, prompt: Prompt) {
        match prompt.ask {
            Ask::Go => {
                let path = prompt
                    .pick
                    .and_then(|i| self.audience.suggestions(&prompt).into_iter().nth(i))
                    .unwrap_or_else(|| prompt.text.trim().to_string());
                if path.is_empty() {
                    return;
                }
                let path = live::absolute(&path);
                let reached = self.audience.send(&Command::Go { path: path.clone() });
                self.sent(&format!("→ {path}"), reached);
            }
            Ask::React => {
                let kind = prompt.pick.unwrap_or(0);
                let reached = self.audience.react(kind);
                self.sent(&format!("{} sent", REACTIONS[kind].0), reached);
            }
            Ask::Chat => {
                let text = prompt.text.trim().to_string();
                let reached = if text.is_empty() {
                    0
                } else {
                    self.audience.say(&text)
                };
                // The box stays open for the next line; a message nobody got stays typed.
                let kept = if reached == 0 {
                    prompt.text
                } else {
                    String::new()
                };
                self.prompt = Some(Prompt {
                    ask: Ask::Chat,
                    text: kept,
                    pick: None,
                });
                if !text.is_empty() && reached == 0 {
                    self.sent("✉ Not sent", 0);
                }
            }
        }
    }

    /// Sends a request of the log again, unless it cannot be sent as it was.
    fn replay(&mut self, n: u32) {
        if let Err(reason) = self.traffic.replay(n) {
            self.toast("✖ Not replayable", reason, false);
        }
    }

    fn sent(&mut self, title: &str, reached: usize) {
        let body = self.audience.reached(reached);
        self.toast(title, body, false);
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

    /// What stays in the scrollback once the terminal is restored.
    fn farewell(&self) {
        if let Some(failure) = &self.boot.failure {
            eprintln!("bunflared: {}", failure.message);
            return;
        }
        let Some(url) = &self
            .boot
            .url
            .as_ref()
            .filter(|_| self.boot.ready_at.is_some())
        else {
            return;
        };
        let paint = |code: &str, text: &str| {
            if self.theme.color {
                format!("\x1b[{code}m{text}\x1b[0m")
            } else {
                text.to_string()
            }
        };
        let badges: Vec<String> = self
            .show
            .unlocked
            .iter()
            .map(|a| format!("★ {}", a.name))
            .collect();
        println!();
        println!(
            "  {}      {}",
            paint("35", r" (\(\ "),
            paint("1", "bunflared · session recap")
        );
        println!(
            "  {}     {}  {}",
            paint("35", r" ( ^.^)/"),
            paint("36", url),
            paint("2", "(closed)")
        );
        println!(
            "  {}    {} live · {} · {} · {}",
            paint("35", r#" o_(")(")"#),
            uptime(self.live_for()),
            plural(self.traffic.total as usize, "request"),
            plural(self.traffic.visitors.len(), "visitor"),
            plural(self.traffic.classes[3] as usize, "error"),
        );
        if !badges.is_empty() {
            println!("               {}", paint("33", &badges.join("  ")));
        }
        if self.audience.feedback > 0 {
            let note = format!(
                "✎ {} saved in {}/",
                plural(self.audience.feedback as usize, "feedback note"),
                notes::FOLDER
            );
            println!("               {}", paint("36", &note));
        }
        println!();
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
