mod art;
mod dashboard;
mod fx;
mod scenes;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::crossterm::event::{
    self, Event as Term, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use tokio::sync::watch;

use crate::clipboard;
use crate::live::{Command, Hub, Pointer};
use crate::proxy::mount;
use crate::share::{Event, Failure, Hit};
use crate::state::{Record, uptime};
use crate::widget::{self, Presence};

const BOOT: f32 = 1.5;
const LAUNCH: f32 = 3.2;
const GOODBYE: f32 = 1.6;
const SNIFF: f32 = 0.35;
pub const LANE_TRIP: f32 = 1.4;
const TOAST_LIFE: f32 = 3.5;
const LOG_KEEP: usize = 200;
const SPARK_KEEP: usize = 120;
pub const SLEEP_AFTER: f32 = 60.0;
const MARATHON: f32 = 3600.0;
const STAMPEDE_PRESSES: usize = 5;
const STAMPEDE_WINDOW: Duration = Duration::from_millis(1500);
const PYRO: u32 = 10;
const PAGES_KEEP: usize = 20;
const SUGGESTIONS: usize = 5;
const CHAT_KEEP: usize = 60;
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
    Goodbye,
    Failed,
}

pub struct PortState {
    pub port: u16,
    pub route: String,
    pub checked: Option<bool>,
    pub up: bool,
    pub hits: u32,
}

pub struct Row {
    pub hit: Hit,
    pub clock: String,
    /// Its rank since the start, a key that survives new rows.
    pub n: u32,
}

pub struct Sprite {
    pub born: Instant,
    pub lane: u16,
    pub glyph: char,
    pub color: Color,
}

pub struct Toast {
    pub title: String,
    pub body: String,
    pub born: Instant,
    pub gold: bool,
}

pub struct Runner {
    pub x: f32,
    pub y: f32,
    pub speed: f32,
}

/// A browser tab reporting through the widget.
pub struct Session {
    pub presence: Presence,
    pub seen: Instant,
    pub first: Instant,
    pub pointer: Option<(Pointer, Instant)>,
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
}

/// A line of the chat panel, sent or received.
pub struct Message {
    pub mine: bool,
    /// The visitor's device, or who a message of ours went to.
    pub who: String,
    pub page: String,
    pub text: String,
    pub clock: String,
}

/// A text box open over the dashboard: while it is, keys are text.
pub struct Prompt {
    pub ask: Ask,
    pub text: String,
    pub pick: Option<usize>,
}

pub struct Qr {
    pub size: usize,
    pub dark: Vec<bool>,
}

pub struct App {
    pub theme: Theme,
    pub phase: Phase,
    pub phase_at: Instant,
    started: Instant,
    pub now: Instant,
    pub dt: f32,
    pub rng: fx::Rng,
    pub particles: fx::Particles,
    pub sprites: Vec<Sprite>,
    pub toasts: Vec<Toast>,
    pub runners: Vec<Runner>,
    pub launch_bursts: u8,

    pub ports: Vec<PortState>,
    pub url: Option<String>,
    pub record: Option<Record>,
    pub qr_code: Option<Qr>,
    /// Particles skip this area: the QR code, the freshly announced link.
    pub keep_clear: Option<Rect>,
    pub resolved: bool,
    pub copied: bool,
    pub progress: f32,
    target: f32,
    pub stage: String,
    pub ready_at: Option<Instant>,
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
    pub failure: Option<Failure>,
    pub unlocked: Vec<&'static art::Achievement>,
    pub sessions: HashMap<String, Session>,
    pub feedback: u32,
    pub hub: Arc<Hub>,
    pub focus: Option<Focus>,
    /// The visitor that commands go to; everyone when `None`.
    pub selected: Option<String>,
    pub picked: Option<u32>,
    pub prompt: Option<Prompt>,
    /// Pages the visitors were on, the most recent first.
    pub pages: Vec<String>,
    pub chat: VecDeque<Message>,

    pub disco: bool,
    pub qr: bool,
    pub help: bool,
    keys: VecDeque<KeyCode>,
    typed: String,
    b_presses: VecDeque<Instant>,
    fireworks: u32,
    pub carrots_until: Option<Instant>,
    pub quitting: Option<Instant>,
    exit: Option<i32>,
}

pub fn run(
    rx: Receiver<Event>,
    stop: &watch::Sender<bool>,
    ports: &[u16],
    theme: Theme,
    hub: Arc<Hub>,
) -> i32 {
    let mut terminal = ratatui::init();
    let mut app = App::new(ports, theme, hub);
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
    fn new(ports: &[u16], theme: Theme, hub: Arc<Hub>) -> Self {
        let now = Instant::now();
        let ports = ports
            .iter()
            .enumerate()
            .map(|(i, &port)| PortState {
                port,
                route: if i == 0 { "/".into() } else { mount(port) },
                checked: None,
                up: true,
                hits: 0,
            })
            .collect();
        Self {
            theme,
            phase: Phase::Boot,
            phase_at: now,
            started: now,
            now,
            dt: 0.0,
            rng: fx::Rng::seeded(),
            particles: fx::Particles::default(),
            sprites: Vec::new(),
            toasts: Vec::new(),
            runners: Vec::new(),
            launch_bursts: 0,
            ports,
            url: None,
            record: None,
            qr_code: None,
            keep_clear: None,
            resolved: false,
            copied: false,
            progress: 0.0,
            target: 0.05,
            stage: "Sniffing around".into(),
            ready_at: None,
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
            failure: None,
            unlocked: Vec::new(),
            sessions: HashMap::new(),
            feedback: 0,
            hub,
            focus: None,
            selected: None,
            picked: None,
            prompt: None,
            pages: Vec::new(),
            chat: VecDeque::new(),
            disco: false,
            qr: false,
            help: false,
            keys: VecDeque::new(),
            typed: String::new(),
            b_presses: VecDeque::new(),
            fireworks: 0,
            carrots_until: None,
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
    }

    /// Animations need 30 fps; a quiet dashboard is fine at 10.
    fn busy(&self) -> bool {
        !self.theme.calm
            && (self.phase != Phase::Dashboard
                || !self.particles.is_empty()
                || !self.sprites.is_empty()
                || !self.toasts.is_empty()
                || !self.runners.is_empty()
                || self.disco
                || self.carrots_until.is_some())
    }

    pub fn live_for(&self) -> u64 {
        self.ready_at.map_or(0, |at| (self.now - at).as_secs())
    }

    pub fn sleeping(&self) -> bool {
        let since = self
            .last_hit
            .or(self.ready_at)
            .map_or(0.0, |at| (self.now - at).as_secs_f32());
        since > SLEEP_AFTER
    }

    pub fn toast(&mut self, title: impl Into<String>, body: impl Into<String>, gold: bool) {
        self.toasts.push(Toast {
            title: title.into(),
            body: body.into(),
            born: self.now,
            gold,
        });
    }

    fn unlock(&mut self, key: &str) {
        let achievement = art::achievement(key);
        if self.unlocked.iter().any(|a| a.key == key) {
            return;
        }
        self.unlocked.push(achievement);
        self.toast(format!("★ {}", achievement.name), achievement.blurb, true);
        if !self.theme.calm {
            let area = self.area();
            let (x, y) = (area.width as f32 / 3.0, area.height as f32 - 4.0);
            self.particles.fireworks(&mut self.rng, x, y);
        }
    }

    pub fn area(&self) -> Rect {
        ratatui::crossterm::terminal::size()
            .map(|(w, h)| Rect::new(0, 0, w, h))
            .unwrap_or(Rect::new(0, 0, 80, 24))
    }

    fn on_event(&mut self, event: Event, now: Instant) {
        self.now = now;
        match event {
            Event::PortChecked { port, ok } => {
                if let Some(state) = self.ports.iter_mut().find(|p| p.port == port) {
                    state.checked = Some(ok);
                    state.up = ok;
                }
            }
            Event::FetchingCloudflared => {
                self.target = self.target.max(0.08);
                self.stage = "Fetching cloudflared, first run only".into();
            }
            Event::TunnelStarting => {
                self.target = self.target.max(0.12);
                self.stage = "Waking up cloudflared".into();
            }
            Event::TunnelUrl(url) => {
                self.url = Some(url);
                self.target = self.target.max(0.35);
                self.stage = "Plugging into Cloudflare's edge".into();
            }
            Event::TunnelRegistered => {
                self.target = self.target.max(0.5);
                self.stage = "Waiting for DNS to notice".into();
            }
            Event::DnsAttempt(attempt) => {
                let dns = 0.5 + 0.45 * (1.0 - 0.85f32.powi(attempt as i32));
                self.target = self.target.max(dns);
                self.stage = format!("Waiting for DNS to notice, attempt {attempt}");
            }
            Event::Ready {
                record,
                resolved,
                copied,
            } => {
                self.url = Some(record.url.clone());
                self.qr_code = qr(&record.url);
                self.record = Some(record);
                self.resolved = resolved;
                self.copied = copied;
                self.ready_at = Some(now);
                self.target = 1.0;
                if local_hour() < 5 {
                    self.unlock("owl");
                }
            }
            Event::Request(hit) => self.on_hit(hit),
            Event::Presence(presence) => {
                self.remember(&presence.page);
                let (first, pointer) = self
                    .sessions
                    .remove(&presence.sid)
                    .map_or((now, None), |s| (s.first, s.pointer));
                let session = Session {
                    presence,
                    seen: now,
                    first,
                    pointer,
                };
                self.sessions.insert(session.presence.sid.clone(), session);
            }
            Event::Feedback(note) => {
                self.feedback += 1;
                let excerpt: String = note.message.chars().take(40).collect();
                self.toast(
                    format!("✎ {} on {}", note.device, note.page),
                    excerpt,
                    false,
                );
                self.unlock("critic");
            }
            Event::Chat(said) => {
                let excerpt: String = said.text.chars().take(40).collect();
                self.toast(
                    format!("✉ {} on {}", said.device, said.page),
                    excerpt,
                    false,
                );
                self.remember_message(Message {
                    mine: false,
                    who: said.device,
                    page: said.page,
                    text: said.text,
                    clock: clock(),
                });
            }
            Event::Pointer(pointer) => {
                if let Some(session) = self.sessions.get_mut(&pointer.sid) {
                    session.pointer = Some((pointer, now));
                }
            }
            Event::PortHealth { port, ok } => {
                if let Some(state) = self.ports.iter_mut().find(|p| p.port == port) {
                    state.up = ok;
                }
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
                    self.failure = Some(failure);
                    self.go(Phase::Failed);
                }
            }
        }
    }

    fn on_hit(&mut self, hit: Hit) {
        self.total += 1;
        let class = match hit.status {
            0..300 => 0,
            300..400 => 1,
            400..500 => 2,
            _ => 3,
        };
        self.classes[class] += 1;
        self.ms_sum += hit.ms as u64;
        if let Some(back) = self.per_second.back_mut() {
            *back += 1;
        }
        if let Some(state) = self.ports.iter_mut().find(|p| p.port == hit.port) {
            state.hits += 1;
        }
        if self.visitors.insert(hit.visitor.clone()) {
            match self.visitors.len() {
                1 => self.unlock("first"),
                5 => self.unlock("party"),
                _ => {}
            }
        }
        match self.total {
            100 => self.unlock("centurion"),
            1000 => self.unlock("thousand"),
            _ => {}
        }
        if hit.ms < 10 {
            self.unlock("speed");
        }
        if hit.upgrade && hit.status == 101 {
            self.sockets += 1;
            self.unlock("tube");
        }
        if hit.status >= 500 {
            self.last_error = Some(self.now);
            self.unlock("survivor");
        }
        self.last_hit = Some(self.now);
        if !self.theme.calm && self.sprites.len() < 80 {
            let (glyph, color) = match (hit.upgrade, class) {
                (true, 0) => ('≈', fx::PURPLE),
                (_, 0) => ('●', fx::GREEN),
                (_, 1) => ('◆', fx::CYAN),
                (_, 2) => ('▲', fx::YELLOW),
                _ => ('✖', fx::RED),
            };
            let lane = self.rng.below(3) as u16;
            self.sprites.push(Sprite {
                born: self.now,
                lane,
                glyph,
                color,
            });
        }
        self.rows.push_front(Row {
            hit,
            clock: clock(),
            n: self.total,
        });
        self.rows.truncate(LOG_KEEP);
    }

    fn tick(&mut self, now: Instant) {
        self.dt = (now - self.now).as_secs_f32().min(0.1);
        self.now = now;
        let t = self.t();
        let calm = self.theme.calm;
        match self.phase {
            Phase::Boot if calm || t >= BOOT => self.go(Phase::Preflight),
            Phase::Preflight => {
                let checked = self.ports.iter().all(|p| p.checked.is_some());
                let shown = calm || t >= 0.3 + SNIFF * self.ports.len() as f32 + 0.4;
                if checked && shown {
                    self.go(Phase::Digging);
                }
            }
            Phase::Digging if self.ready_at.is_some() => self.go(if calm {
                Phase::Dashboard
            } else {
                Phase::Launch
            }),
            Phase::Launch if t >= LAUNCH => self.go(Phase::Dashboard),
            Phase::Goodbye if calm || t >= GOODBYE => self.exit = Some(0),
            _ => {}
        }
        if self
            .ready_at
            .is_some_and(|at| (now - at).as_secs_f32() >= MARATHON)
        {
            self.unlock("marathon");
        }

        if self.ready_at.is_none() {
            // Creep between milestones so the bar never looks stuck.
            self.target = (self.target + self.dt * 0.01).min(0.97);
        }
        self.progress += (self.target - self.progress) * (self.dt * 3.0).min(1.0);
        while now.duration_since(self.bucket_at) >= Duration::from_secs(1) {
            self.bucket_at += Duration::from_secs(1);
            self.per_second.push_back(0);
            if self.per_second.len() > SPARK_KEEP {
                self.per_second.pop_front();
            }
        }
        self.particles.step(self.dt);
        self.sprites
            .retain(|s| (now - s.born).as_secs_f32() < LANE_TRIP);
        self.toasts
            .retain(|toast| (now - toast.born).as_secs_f32() < TOAST_LIFE);
        let dt = self.dt;
        for runner in &mut self.runners {
            runner.x -= runner.speed * dt;
        }
        self.runners.retain(|runner| runner.x > -12.0);
        if self.carrots_until.is_some_and(|until| now >= until) {
            self.carrots_until = None;
        }
    }

    fn on_key(&mut self, key: KeyEvent, stop: &watch::Sender<bool>) {
        let ctrl_c =
            key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
        if self.prompt.is_some() && self.phase == Phase::Dashboard && !ctrl_c {
            return self.on_prompt_key(key);
        }
        let quit = ctrl_c || key.code == KeyCode::Char('q');
        match self.phase {
            Phase::Goodbye => {
                if quit {
                    self.exit = Some(0);
                }
                return;
            }
            Phase::Failed => {
                self.exit = Some(self.failure.as_ref().map_or(1, |f| f.code));
                return;
            }
            _ if quit => {
                self.quitting = Some(self.now);
                self.help = false;
                self.qr = false;
                self.prompt = None;
                let _ = stop.send(true);
                self.go(Phase::Goodbye);
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
            self.disco = !self.disco;
            self.keys.clear();
            self.unlock("disco");
        }
        if let KeyCode::Char(ch) = key.code {
            self.typed.push(ch);
            if self.typed.len() > 16 {
                self.typed.remove(0);
            }
            if self.typed.ends_with(CARROT_WORD) {
                self.carrots_until = Some(self.now + Duration::from_secs(5));
                self.typed.clear();
                self.unlock("carrot");
            }
        }

        let dashboard = self.phase == Phase::Dashboard;
        match key.code {
            KeyCode::Esc if self.help || self.qr => {
                self.help = false;
                self.qr = false;
            }
            KeyCode::Esc => {
                self.focus = None;
                self.select(None);
                self.picked = None;
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
            KeyCode::Char('R') if dashboard => {
                let reached = self.hub.send(self.selected.as_deref(), &Command::Reload);
                self.sent("↻ Reload", reached);
            }
            KeyCode::Char('?') => self.help = !self.help,
            KeyCode::Char('r') if self.qr_code.is_some() => self.qr = !self.qr,
            KeyCode::Char('c') => {
                if let Some(url) = &self.url {
                    self.copied = clipboard::copy(url);
                    let body = if self.copied {
                        "Paste it anywhere."
                    } else {
                        "No clipboard tool found."
                    };
                    self.toast("Link copied", body, false);
                }
            }
            KeyCode::Char('o') => {
                if let Some(url) = self.url.clone().filter(|_| self.ready_at.is_some()) {
                    clipboard::open(&url);
                    self.toast("Opening your browser", "Say hi to the bunny.", false);
                }
            }
            KeyCode::Char('f') if !self.theme.calm => {
                let area = self.area();
                let x = self.rng.range(8.0, area.width.saturating_sub(8) as f32);
                let y = self
                    .rng
                    .range(3.0, area.height.saturating_sub(4).max(4) as f32);
                self.particles.fireworks(&mut self.rng, x, y);
                self.fireworks += 1;
                if self.fireworks == PYRO {
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
                    self.stampede();
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
                    .map(|s| s.presence.sid.clone())
                    .collect();
                let next = stepped(&sids, self.selected.as_ref(), down);
                self.select(next);
            }
            Focus::Log => {
                let ranks: Vec<u32> = self.rows.iter().map(|row| row.n).collect();
                self.picked = stepped(&ranks, self.picked.as_ref(), down);
            }
        }
    }

    /// The selected visitor is the one whose pointer the radar follows.
    fn select(&mut self, sid: Option<String>) {
        self.hub.follow(sid.as_deref());
        self.selected = sid;
    }

    fn on_prompt_key(&mut self, key: KeyEvent) {
        let count = self
            .prompt
            .as_ref()
            .map_or(0, |prompt| self.suggestions(prompt).len());
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
                    .and_then(|i| self.suggestions(&prompt).into_iter().nth(i))
                    .unwrap_or_else(|| prompt.text.trim().to_string());
                if path.is_empty() {
                    return;
                }
                let path = if path.starts_with('/') {
                    path
                } else {
                    format!("/{path}")
                };
                let command = Command::Go { path: path.clone() };
                let reached = self.hub.send(self.selected.as_deref(), &command);
                self.sent(&format!("→ {path}"), reached);
            }
            Ask::Chat => {
                let text = prompt.text.trim().to_string();
                let to = self.target();
                let reached = if text.is_empty() {
                    0
                } else {
                    self.hub.say(self.selected.as_deref(), &to, &text)
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
                if text.is_empty() {
                    return;
                }
                if reached == 0 {
                    self.sent("✉ Not sent", 0);
                    return;
                }
                self.remember_message(Message {
                    mine: true,
                    who: to,
                    page: String::new(),
                    text,
                    clock: clock(),
                });
            }
        }
    }

    fn remember_message(&mut self, message: Message) {
        self.chat.push_back(message);
        if self.chat.len() > CHAT_KEEP {
            self.chat.pop_front();
        }
    }

    /// The pages offered under the go box, filtered by what is typed.
    pub fn suggestions(&self, prompt: &Prompt) -> Vec<String> {
        if prompt.ask != Ask::Go {
            return Vec::new();
        }
        let text = prompt.text.trim();
        self.pages
            .iter()
            .filter(|page| page.contains(text))
            .take(SUGGESTIONS)
            .cloned()
            .collect()
    }

    fn remember(&mut self, page: &str) {
        self.pages.retain(|seen| seen != page);
        self.pages.insert(0, page.to_string());
        self.pages.truncate(PAGES_KEEP);
    }

    /// Who commands go to, for titles and toasts.
    pub fn target(&self) -> String {
        self.selected
            .as_ref()
            .and_then(|sid| self.sessions.get(sid))
            .map_or_else(|| "everyone".into(), |s| s.presence.device.clone())
    }

    fn sent(&mut self, title: &str, reached: usize) {
        let body = match reached {
            0 => "No page is connected live.".to_string(),
            n => format!("{}, {}.", self.target(), plural(n, "page")),
        };
        self.toast(title, body, false);
    }

    fn stampede(&mut self) {
        let area = self.area();
        for _ in 0..10 {
            let x = area.width as f32 + self.rng.range(0.0, 40.0);
            let y = self
                .rng
                .range(1.0, area.height.saturating_sub(4).max(2) as f32);
            let speed = self.rng.range(25.0, 55.0);
            self.runners.push(Runner { x, y, speed });
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        self.keep_clear = None;
        match self.phase {
            Phase::Boot => scenes::boot(self, frame),
            Phase::Preflight => scenes::preflight(self, frame),
            Phase::Digging => scenes::digging(self, frame),
            Phase::Launch => scenes::launch(self, frame),
            Phase::Dashboard => dashboard::render(self, frame),
            Phase::Goodbye => scenes::goodbye(self, frame),
            Phase::Failed => scenes::failed(self, frame),
        }
        if !self.theme.calm {
            scenes::fun(self, frame);
            self.particles
                .draw(frame.buffer_mut(), &self.theme, self.keep_clear);
        }
        dashboard::overlays(self, frame);
    }

    /// What stays in the scrollback once the terminal is restored.
    fn farewell(&self) {
        if let Some(failure) = &self.failure {
            eprintln!("bunflared: {}", failure.message);
            return;
        }
        let Some(url) = &self.url.as_ref().filter(|_| self.ready_at.is_some()) else {
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
            plural(self.total as usize, "request"),
            plural(self.visitors.len(), "visitor"),
            plural(self.classes[3] as usize, "error"),
        );
        if !badges.is_empty() {
            println!("               {}", paint("33", &badges.join("  ")));
        }
        if self.feedback > 0 {
            let note = format!(
                "✎ {} saved in {}/",
                plural(self.feedback as usize, "feedback note"),
                widget::FOLDER
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

fn qr(url: &str) -> Option<Qr> {
    const QUIET: usize = 2;
    let code = qrcode::QrCode::with_error_correction_level(url, qrcode::EcLevel::L).ok()?;
    let width = code.width();
    let colors = code.to_colors();
    let size = width + QUIET * 2;
    let mut dark = vec![false; size * size];
    for y in 0..width {
        for x in 0..width {
            dark[(y + QUIET) * size + x + QUIET] = colors[y * width + x] == qrcode::Color::Dark;
        }
    }
    Some(Qr { size, dark })
}

fn local_hour() -> u32 {
    chrono::Timelike::hour(&chrono::Local::now())
}

fn clock() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}
