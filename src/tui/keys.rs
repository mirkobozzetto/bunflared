use std::collections::VecDeque;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::watch;

use super::{App, Phase, dashboard, stepped};
use crate::clipboard;
use crate::live::{self, Command, REACTIONS};

const STAMPEDE_PRESSES: usize = 5;
const STAMPEDE_WINDOW: Duration = Duration::from_millis(1500);
// No command key in it, or typing it would copy, open or quit.
const CARROT_WORD: &str = "yum";
const CARROTS_FOR: Duration = Duration::from_secs(5);
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
    Filter,
}

/// A text box open over the dashboard: while it is, keys are text.
pub struct Prompt {
    pub ask: Ask,
    pub text: String,
    pub pick: Option<usize>,
}

impl Prompt {
    /// Empty; the reactions start on the first one.
    fn new(ask: Ask) -> Self {
        Self {
            ask,
            text: String::new(),
            pick: (ask == Ask::React).then_some(0),
        }
    }
}

/// The keyboard: the focused panel, the box and overlays open, the codes typed.
#[derive(Default)]
pub struct Keys {
    pub focus: Option<Focus>,
    pub prompt: Option<Prompt>,
    pub qr: bool,
    pub help: bool,
    pub quitting: Option<Instant>,
    /// The last keys, for the Konami code.
    recent: VecDeque<KeyCode>,
    typed: String,
    b_presses: VecDeque<Instant>,
}

impl App {
    pub(super) fn on_key(&mut self, key: KeyEvent, stop: &watch::Sender<bool>) {
        let ctrl_c =
            key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
        if self.keys.prompt.is_some() && self.phase == Phase::Dashboard && !ctrl_c {
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
            _ if quit => return self.quit(stop),
            Phase::Boot => return self.go(Phase::Preflight),
            Phase::Launch => return self.go(Phase::Dashboard),
            _ => {}
        }
        self.on_secret_key(key.code);
        match self.traffic.details {
            Some(n) if self.phase == Phase::Dashboard => self.on_details_key(key.code, n),
            _ => self.on_command_key(key.code),
        }
    }

    /// The dashboard dissolves, the goodbye plays and sharing stops.
    fn quit(&mut self, stop: &watch::Sender<bool>) {
        self.keys.quitting = Some(self.now);
        self.keys.help = false;
        self.keys.qr = false;
        self.keys.prompt = None;
        self.traffic.details = None;
        let _ = stop.send(true);
        let leaving = if self.phase == Phase::Dashboard && !self.theme.calm {
            self.show.shaders.dissolve();
            Phase::Dissolve
        } else {
            Phase::Goodbye
        };
        self.go(leaving);
    }

    /// The Konami code and the carrot word, on every screen.
    fn on_secret_key(&mut self, code: KeyCode) {
        let recent = &mut self.keys.recent;
        recent.push_back(code);
        if recent.len() > KONAMI.len() {
            recent.pop_front();
        }
        if recent.iter().eq(KONAMI.iter()) {
            recent.clear();
            self.show.disco = !self.show.disco && !self.theme.calm;
            self.show.shaders.disco(self.show.disco);
            self.unlock("disco");
        }
        if let KeyCode::Char(ch) = code {
            let typed = &mut self.keys.typed;
            typed.push(ch);
            if typed.len() > 16 {
                typed.remove(0);
            }
            if typed.ends_with(CARROT_WORD) {
                typed.clear();
                self.show.carrots_until = Some(self.now + CARROTS_FOR);
                self.unlock("carrot");
            }
        }
    }

    /// While a request's details are open.
    fn on_details_key(&mut self, code: KeyCode, n: u32) {
        let scroll = self.traffic.details_scroll;
        match code {
            KeyCode::Esc | KeyCode::Enter => self.traffic.details = None,
            KeyCode::Up => self.traffic.details_scroll = scroll.saturating_sub(1),
            KeyCode::Down => self.traffic.details_scroll = scroll.saturating_add(1),
            KeyCode::PageUp => self.traffic.details_scroll = scroll.saturating_sub(10),
            KeyCode::PageDown => self.traffic.details_scroll = scroll.saturating_add(10),
            KeyCode::Char('p') => self.replay(n),
            KeyCode::Char('c') => match self.traffic.curl(n) {
                Some(Ok(curl)) if clipboard::copy(&curl) => {
                    self.toast("Copied as curl", "It goes to your local server.", false)
                }
                Some(Ok(_)) => self.toast("✖ Not copied", "No clipboard tool found.", false),
                Some(Err(reason)) => self.toast("✖ No curl for it", reason, false),
                None => {}
            },
            _ => {}
        }
    }

    /// The dashboard's commands, and the keys that work on every screen.
    fn on_command_key(&mut self, code: KeyCode) {
        let dashboard = self.phase == Phase::Dashboard;
        match code {
            KeyCode::Esc if self.keys.help || self.keys.qr => {
                self.keys.help = false;
                self.keys.qr = false;
            }
            KeyCode::Esc => {
                self.keys.focus = None;
                self.audience.select(None);
                self.traffic.picked = None;
            }
            KeyCode::Tab if dashboard => {
                self.keys.focus = Some(match self.keys.focus {
                    Some(Focus::Visitors) => Focus::Log,
                    _ => Focus::Visitors,
                });
            }
            KeyCode::Up | KeyCode::Down if dashboard => self.step(code == KeyCode::Down),
            KeyCode::Char('g') if dashboard => self.keys.prompt = Some(Prompt::new(Ask::Go)),
            KeyCode::Char('m') if dashboard => self.keys.prompt = Some(Prompt::new(Ask::Chat)),
            KeyCode::Enter if dashboard && self.keys.focus == Some(Focus::Log) => {
                if self.traffic.picked.is_some() {
                    self.traffic.details = self.traffic.picked;
                    self.traffic.details_scroll = 0;
                }
            }
            KeyCode::Char('p') if dashboard => match self.traffic.picked {
                Some(n) => self.replay(n),
                None => self.toast("Pick a request first", "Tab, then ↑ ↓.", false),
            },
            KeyCode::Char('e') if dashboard => self.keys.prompt = Some(Prompt::new(Ask::React)),
            KeyCode::Char('/') if dashboard => {
                let text = self.traffic.filter.clone();
                self.keys.prompt = Some(Prompt {
                    text,
                    ..Prompt::new(Ask::Filter)
                });
            }
            KeyCode::Char('x') if dashboard => self.audience.chat.clear(),
            KeyCode::Char('R') if dashboard => {
                let reached = self.audience.send(&Command::Reload);
                self.sent("↻ Reload", reached);
            }
            KeyCode::Char('?') => self.keys.help = !self.keys.help,
            // The code already on screen flashes instead of opening twice.
            KeyCode::Char('r') if self.boot.qr_code.is_some() => {
                if self.keys.qr || self.show.spots.qr.is_none() {
                    self.keys.qr = !self.keys.qr;
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
                let presses = &mut self.keys.b_presses;
                presses.push_back(self.now);
                while presses
                    .front()
                    .is_some_and(|at| self.now - *at > STAMPEDE_WINDOW)
                {
                    presses.pop_front();
                }
                if presses.len() >= STAMPEDE_PRESSES {
                    presses.clear();
                    self.show.stampede();
                    self.unlock("stampede");
                }
            }
            _ => {}
        }
    }

    /// Moves the selection in the focused panel, the visitors by default.
    fn step(&mut self, down: bool) {
        match *self.keys.focus.get_or_insert(Focus::Visitors) {
            Focus::Visitors => {
                let sids: Vec<String> = dashboard::roster(self)
                    .iter()
                    .map(|v| v.presence.sid.clone())
                    .collect();
                let next = stepped(&sids, self.audience.selected.as_ref(), down);
                self.audience.select(next);
            }
            Focus::Log => {
                let ranks: Vec<u32> = self.traffic.shown().map(|row| row.n).collect();
                self.traffic.picked = stepped(&ranks, self.traffic.picked.as_ref(), down);
            }
        }
    }

    /// While a text box is open, keys are text.
    fn on_prompt_key(&mut self, key: KeyEvent) {
        let count = self
            .keys
            .prompt
            .as_ref()
            .map_or(0, |prompt| self.audience.suggestions(prompt).len());
        let Some(prompt) = self.keys.prompt.as_mut() else {
            return;
        };
        let digit = match key.code {
            KeyCode::Char(ch) => ch.to_digit(10).map(|d| d as usize),
            _ => None,
        };
        if prompt.ask == Ask::React {
            if let Some(n) = digit.filter(|n| (1..=REACTIONS.len()).contains(n)) {
                prompt.pick = Some(n - 1);
                if let Some(prompt) = self.keys.prompt.take() {
                    self.submit(prompt);
                }
            } else if let KeyCode::Char(_) = key.code {
                return;
            }
        }
        let Some(prompt) = self.keys.prompt.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc => {
                if prompt.ask == Ask::Filter {
                    self.traffic.filter.clear();
                }
                self.keys.prompt = None;
            }
            KeyCode::Enter => {
                if let Some(prompt) = self.keys.prompt.take() {
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
            Ask::Filter => {
                self.traffic.filter = prompt.text.trim().to_string();
                self.traffic.picked = None;
                self.keys.focus = Some(Focus::Log);
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
                self.keys.prompt = Some(Prompt {
                    text: kept,
                    ..Prompt::new(Ask::Chat)
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
}
