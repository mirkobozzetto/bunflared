//! How the home screen looks.

use std::path::Path;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

use super::{Home, Row};
use crate::ports::Listener;
use crate::proxy::mount;
use crate::state::uptime;
use crate::tui::{Theme, fx};

const PROGRAM_WIDTH: usize = 14;
const AGE_WIDTH: usize = 8;
const ROUTE_WIDTH: usize = 12;
const MIN_FOLDER: usize = 12;

impl Home {
    pub(super) fn render(&self, frame: &mut Frame, theme: &Theme) {
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
        let mut lines = vec![
            Line::styled(" What do you want to share?", bold),
            Line::raw(""),
        ];
        if let Some(last) = &self.last {
            let missing = self.missing(last);
            let mut spans = vec![
                Span::raw("   ↻ same as last time   "),
                Span::styled(last.describe(), theme.fg(fx::CYAN)),
            ];
            if !missing.is_empty() {
                let gone = format!("   not listening: {}", missing.join(", "));
                spans.push(Span::styled(gone, theme.fg(fx::RED)));
            }
            lines.push(self.cursored(Line::from(spans), Row::Last));
            lines.push(Line::raw(""));
        }
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
        let (on, off) = (
            theme.fg(fx::GREEN).add_modifier(Modifier::BOLD),
            theme.fg(fx::DIM),
        );
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
            None if self.cursor == Row::Last => {
                Line::styled(" Start these apps first, or pick below.", theme.fg(fx::DIM))
            }
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
