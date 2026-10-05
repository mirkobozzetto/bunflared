//! The screen `bunf` opens without a port: the apps listening here, which to
//! share and how, and the command that would do the same.

use std::collections::BTreeMap;
use std::fs;
use std::ops::ControlFlow::{self, Break, Continue};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use serde::{Deserialize, Serialize};

use super::{Theme, stepped};
use crate::ports::{self, Listener};

mod view;

const FRAME: Duration = Duration::from_millis(100);
const RESCAN: Duration = Duration::from_secs(1);

/// What to share and how, as the command's flags say it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub ports: Vec<u16>,
    pub local: bool,
    pub widget: bool,
    pub calm: bool,
}

impl Choice {
    pub fn command(&self) -> String {
        let mut words = vec![program()];
        words.extend(self.ports.iter().map(u16::to_string));
        words.extend(self.local.then(|| "--local".to_string()));
        words.extend((!self.widget).then(|| "--no-widget".to_string()));
        words.extend(self.calm.then(|| "--calm".to_string()));
        words.join(" ")
    }

    fn describe(&self) -> String {
        let ports: Vec<String> = self.ports.iter().map(u16::to_string).collect();
        let place = if self.local {
            "on this machine"
        } else {
            "public link"
        };
        let mut words = vec![ports.join(", "), place.to_string()];
        words.extend((!self.widget).then(|| "no feedback button".to_string()));
        words.extend(self.calm.then(|| "no animations".to_string()));
        words.join(" · ")
    }
}

/// The last choice made from the home screen, by folder, beside the share
/// records and never in the project.
fn memory_file() -> Option<PathBuf> {
    Some(crate::os::data_dir()?.join("home").join("last.json"))
}

fn memory() -> BTreeMap<String, Choice> {
    memory_file()
        .and_then(|path| fs::read(path).ok())
        .and_then(|json| serde_json::from_slice(&json).ok())
        .unwrap_or_default()
}

fn remember(here: &Path, choice: &Choice) {
    let Some(path) = memory_file() else {
        return;
    };
    let mut memory = memory();
    memory.insert(here.to_string_lossy().into_owned(), choice.clone());
    if let (Some(dir), Ok(json)) = (path.parent(), serde_json::to_vec_pretty(&memory)) {
        let _ = fs::create_dir_all(dir).and_then(|_| fs::write(&path, json));
    }
}

/// The name it was started under, bunf or bunflared.
fn program() -> String {
    std::env::args_os()
        .next()
        .as_deref()
        .map(Path::new)
        .and_then(Path::file_stem)
        .map_or("bunflared".into(), |name| {
            name.to_string_lossy().into_owned()
        })
}

#[derive(Clone, Copy, PartialEq)]
enum Row {
    Last,
    Port(u16),
    Widget,
    Animations,
}

struct Home {
    apps: Vec<Listener>,
    here: PathBuf,
    /// In the order they were checked: the first is served at `/`.
    checked: Vec<u16>,
    cursor: Row,
    local: bool,
    widget: bool,
    calm: bool,
    /// Nothing checked yet: the next app found gets checked.
    fresh: bool,
    last: Option<Choice>,
}

/// Shows the screen until Enter launches a share or q leaves without one.
pub fn run(theme: &Theme, start: Choice) -> Option<Choice> {
    let here = std::env::current_dir().unwrap_or_default();
    let last = memory().remove(here.to_string_lossy().as_ref());
    let mut home = Home {
        apps: Vec::new(),
        checked: Vec::new(),
        cursor: Row::Widget,
        local: start.local || last.as_ref().is_some_and(|last| last.local),
        widget: start.widget,
        calm: start.calm,
        fresh: true,
        here,
        last,
    };
    home.update(ports::scan());
    // `-l` asks for this machine: a last public share is not what Enter does.
    if home
        .last
        .as_ref()
        .is_some_and(|last| home.missing(last).is_empty() && (last.local || !start.local))
    {
        home.cursor = Row::Last;
    }
    let scans = watch();
    let mut terminal = ratatui::init();
    let result = loop {
        if let Some(apps) = scans.try_iter().last() {
            home.update(apps);
        }
        if terminal.draw(|frame| home.render(frame, theme)).is_err() {
            break None;
        }
        match event::poll(FRAME) {
            Err(_) => break None,
            Ok(false) => continue,
            Ok(true) => {}
        }
        if let Ok(Event::Key(key)) = event::read()
            && key.kind == KeyEventKind::Press
            && let Break(result) = home.key(key)
        {
            break result;
        }
    };
    ratatui::restore();
    if let Some(choice) = &result {
        remember(&home.here, choice);
    }
    result
}

/// A fresh scan every second, until the receiver is gone.
fn watch() -> Receiver<Vec<Listener>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(RESCAN);
            if tx.send(ports::scan()).is_err() {
                break;
            }
        }
    });
    rx
}

impl Home {
    fn update(&mut self, mut apps: Vec<Listener>) {
        let here = &self.here;
        // The apps started from this folder first, then the most recent.
        apps.sort_by_key(|app| (distance(app, here), app.age.unwrap_or(u64::MAX), app.port));
        self.apps = apps;
        if self.fresh
            && let Some(first) = self.apps.first().map(|app| app.port)
        {
            self.checked.push(first);
            self.fresh = false;
            self.cursor = Row::Port(first);
        }
        if let Row::Port(port) = self.cursor
            && !self.listening(port)
        {
            self.cursor = self.rows()[0];
        }
    }

    fn listening(&self, port: u16) -> bool {
        self.apps.iter().any(|app| app.port == port)
    }

    fn missing(&self, choice: &Choice) -> Vec<String> {
        let ports = choice.ports.iter().filter(|&&port| !self.listening(port));
        ports.map(u16::to_string).collect()
    }

    /// The checked ports still listening, the one served at `/` first.
    fn ports(&self) -> Vec<u16> {
        let ports = self.checked.iter().copied();
        ports.filter(|&port| self.listening(port)).collect()
    }

    fn rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = self.last.iter().map(|_| Row::Last).collect();
        rows.extend(self.apps.iter().map(|app| Row::Port(app.port)));
        rows.extend([Row::Widget, Row::Animations]);
        rows
    }

    /// What Enter launches, if anything.
    fn pending(&self) -> Option<Choice> {
        if self.cursor == Row::Last {
            let last = self.last.as_ref();
            return last.filter(|last| self.missing(last).is_empty()).cloned();
        }
        let ports = self.ports();
        (!ports.is_empty()).then_some(Choice {
            ports,
            local: self.local,
            widget: self.widget,
            calm: self.calm,
        })
    }

    fn key(&mut self, key: KeyEvent) -> ControlFlow<Option<Choice>> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Break(None),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Break(None);
            }
            KeyCode::Enter => {
                if let Some(choice) = self.pending() {
                    return Break(Some(choice));
                }
            }
            KeyCode::Up | KeyCode::Char('k') => self.step(false),
            KeyCode::Down | KeyCode::Char('j') => self.step(true),
            KeyCode::Tab | KeyCode::BackTab => {
                // The last time's line launches as it was: editing leaves it.
                if self.cursor == Row::Last {
                    self.cursor = self.rows()[1];
                }
                self.local = !self.local;
            }
            KeyCode::Char(' ') => self.toggle(),
            _ => {}
        }
        Continue(())
    }

    fn step(&mut self, down: bool) {
        if let Some(row) = stepped(&self.rows(), Some(&self.cursor), down) {
            self.cursor = row;
        }
    }

    fn toggle(&mut self) {
        match self.cursor {
            Row::Port(port) => {
                match self.checked.iter().position(|&p| p == port) {
                    Some(at) => {
                        self.checked.remove(at);
                    }
                    None => self.checked.push(port),
                }
                self.fresh = false;
            }
            Row::Widget => self.widget = !self.widget,
            Row::Animations => self.calm = !self.calm,
            Row::Last => {}
        }
    }
}

/// 0 started in this very folder, 1 below it, 2 anywhere else.
fn distance(app: &Listener, here: &Path) -> u8 {
    match app.folder.as_deref() {
        Some(folder) if folder == here => 0,
        Some(folder) if folder.starts_with(here) => 1,
        _ => 2,
    }
}
