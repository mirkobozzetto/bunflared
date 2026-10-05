//! The screen `bunf` opens without a port: the apps listening here, which to
//! share and how, and the command that would do the same.

use std::ops::ControlFlow::{self, Break, Continue};
use std::path::{Path, PathBuf};
use std::time::Duration;

use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

use super::{Theme, fx, stepped};
use crate::ports::{self, Listener};
use crate::proxy::mount;
use crate::state::uptime;

const FRAME: Duration = Duration::from_millis(100);
const PROGRAM_WIDTH: usize = 14;
const AGE_WIDTH: usize = 8;
const ROUTE_WIDTH: usize = 12;
const MIN_FOLDER: usize = 12;

/// What to share and how, as the command's flags say it.
#[derive(Clone, Debug, PartialEq)]
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
}

/// The name it was started under, bunf or bunflared.
fn program() -> String {
    std::env::args_os()
        .next()
        .as_deref()
        .map(Path::new)
        .and_then(Path::file_stem)
        .map_or("bunflared".into(), |name| name.to_string_lossy().into_owned())
}

#[derive(Clone, Copy, PartialEq)]
enum Row {
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
}

/// Shows the screen until Enter launches a share or q leaves without one.
pub fn run(theme: &Theme, start: Choice) -> Option<Choice> {
    let mut home = Home {
        apps: Vec::new(),
        here: std::env::current_dir().unwrap_or_default(),
        checked: Vec::new(),
        cursor: Row::Widget,
        local: start.local,
        widget: start.widget,
        calm: start.calm,
        fresh: true,
    };
    home.update(ports::scan());
    let mut terminal = ratatui::init();
    let result = loop {
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
    result
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

    /// The checked ports still listening, the one served at `/` first.
    fn ports(&self) -> Vec<u16> {
        let ports = self.checked.iter().copied();
        ports.filter(|&port| self.listening(port)).collect()
    }

    fn rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = self.apps.iter().map(|app| Row::Port(app.port)).collect();
        rows.extend([Row::Widget, Row::Animations]);
        rows
    }

    /// What Enter launches, if anything.
    fn pending(&self) -> Option<Choice> {
        let ports = self.ports();
        (!ports.is_empty()).then(|| Choice {
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
            KeyCode::Tab | KeyCode::BackTab => self.local = !self.local,
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
            Row::Port(port) => match self.checked.iter().position(|&p| p == port) {
                Some(at) => {
                    self.checked.remove(at);
                }
                None => self.checked.push(port),
            },
            Row::Widget => self.widget = !self.widget,
            Row::Animations => self.calm = !self.calm,
        }
        self.fresh = false;
    }

    fn render(&self, frame: &mut Frame, theme: &Theme) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(theme.fg(fx::DIM))
            .title(Span::styled(
                " bunflared ",
                theme.fg(fx::PINK).add_modifier(Modifier::BOLD),
            ));
        let inner = block.inner(frame.area());
        frame.render_widget(block, frame.area());
        let [body, footer] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).areas(inner);

        let bold = Style::new().add_modifier(Modifier::BOLD);
        let mut lines = vec![Line::styled(" What do you want to share?", bold), Line::raw("")];
        if self.apps.is_empty() {
            lines.push(Line::styled(
                "   Start your app, I'll see it arrive.",
                theme.fg(fx::YELLOW),
            ));
        }
        let ports = self.ports();
        let folders: Vec<String> = self.apps.iter().map(|app| self.folder(app)).collect();
        let longest = folders.iter().map(|f| f.chars().count()).max().unwrap_or(0);
        let folder_width = (inner.width as usize)
            .saturating_sub(4 + 4 + 7 + PROGRAM_WIDTH + 2 + AGE_WIDTH + 2 + ROUTE_WIDTH + 2)
            .max(MIN_FOLDER)
            .min(longest);
        for (app, folder) in self.apps.iter().zip(folders) {
            let route = match ports.iter().position(|&port| port == app.port) {
                Some(0) => "/".to_string(),
                Some(_) => mount(app.port),
                None => String::new(),
            };
            let age = app.age.map(uptime).unwrap_or_default();
            let line = Line::from(vec![
                Span::raw(format!("   {} ", tick(!route.is_empty()))),
                Span::styled(format!("{:<5}  ", app.port), theme.fg(fx::CYAN)),
                Span::raw(format!("{:<PROGRAM_WIDTH$.PROGRAM_WIDTH$}  ", app.program)),
                Span::styled(
                    format!("{:<folder_width$}  ", fit(&folder, folder_width)),
                    theme.fg(fx::DIM),
                ),
                Span::styled(format!("{age:>AGE_WIDTH$}  "), theme.fg(fx::DIM)),
                Span::styled(route, theme.fg(fx::GREEN)),
            ]);
            lines.push(self.cursored(line, Row::Port(app.port)));
        }

        lines.push(Line::raw(""));
        let (on, off) = (theme.fg(fx::GREEN).add_modifier(Modifier::BOLD), theme.fg(fx::DIM));
        let (public, local) = if self.local { (off, on) } else { (on, off) };
        lines.push(Line::from(vec![
            Span::raw("   Where   "),
            Span::styled(format!("{} public link", dot(!self.local)), public),
            Span::raw("   "),
            Span::styled(format!("{} on this machine", dot(self.local)), local),
            Span::styled("   tab switches", theme.fg(fx::DIM)),
        ]));
        lines.push(Line::raw(""));
        let option = |on: bool, label: &str| Line::raw(format!("   {} {label}", tick(on)));
        lines.push(self.cursored(option(self.widget, "feedback button"), Row::Widget));
        lines.push(self.cursored(option(!self.calm, "animations"), Row::Animations));
        frame.render_widget(Paragraph::new(lines), body);

        let command = match self.pending() {
            Some(choice) => Line::styled(format!(" $ {}", choice.command()), bold),
            None => Line::styled(" Check an app with space.", theme.fg(fx::DIM)),
        };
        let keys = Line::styled(
            " enter share · space check · tab where · ↑↓ move · q quit",
            theme.fg(fx::DIM),
        );
        frame.render_widget(Paragraph::new(vec![command, keys]), footer);
    }

    /// Where it runs, from here when it is under here.
    fn folder(&self, app: &Listener) -> String {
        let Some(folder) = app.folder.as_deref() else {
            return String::new();
        };
        if let Ok(rest) = folder.strip_prefix(&self.here) {
            return Path::new(".").join(rest).display().to_string();
        }
        match crate::os::home().and_then(|home| folder.strip_prefix(home).ok()) {
            Some(rest) => Path::new("~").join(rest).display().to_string(),
            None => folder.display().to_string(),
        }
    }

    fn cursored<'a>(&self, line: Line<'a>, row: Row) -> Line<'a> {
        if self.cursor == row {
            line.style(Style::new().add_modifier(Modifier::REVERSED))
        } else {
            line
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

fn tick(on: bool) -> &'static str {
    if on { "[x]" } else { "[ ]" }
}

fn dot(on: bool) -> &'static str {
    if on { "●" } else { "○" }
}

/// The end of `text` when it is too long: the end of a path says the most.
fn fit(text: &str, width: usize) -> String {
    let count = text.chars().count();
    if count <= width {
        return text.to_string();
    }
    let tail: String = text.chars().skip(count + 1 - width).collect();
    format!("…{tail}")
}

