use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::{focusable, highlight, scroll, status_color};
use crate::tui::App;
use crate::tui::fx;
use crate::tui::keys::Focus;
use crate::tui::traffic::Replay;

pub fn draw_log(app: &App, frame: &mut Frame, area: Rect) {
    let block = focusable(app, "requests", Focus::Log);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let theme = &app.theme;
    if app.traffic.rows.is_empty() {
        let waiting = Line::from(Span::styled(
            "No visitors yet. Send the link to someone!",
            theme.fg(fx::DIM),
        ))
        .centered();
        let [_, middle, _] = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(inner);
        frame.render_widget(Paragraph::new(waiting), middle);
        return;
    }
    let at = app
        .traffic
        .rows
        .iter()
        .position(|row| Some(row.n) == app.traffic.picked);
    let lines: Vec<Line> = app
        .traffic
        .rows
        .iter()
        .skip(scroll(at, inner.height as usize))
        .take(inner.height as usize)
        .map(|row| {
            let hit = &row.hit;
            let replayed = match row.replayed {
                None => Span::raw(""),
                Some(Replay::Pending) => Span::styled("↻ …  ", theme.fg(fx::DIM)),
                Some(Replay::Done { status, .. }) => Span::styled(
                    format!("↻ {status} "),
                    theme.fg(status_color(status)).add_modifier(Modifier::BOLD),
                ),
                Some(Replay::Failed) => Span::styled("↻ ✖   ", theme.fg(fx::RED)),
            };
            let line = Line::from(vec![
                Span::styled(format!("{} ", row.clock), theme.fg(fx::DIM)),
                Span::styled(format!("{:<7}", hit.method), theme.fg(fx::PURPLE)),
                Span::styled(
                    format!("{} ", hit.status),
                    theme
                        .fg(status_color(hit.status))
                        .add_modifier(Modifier::BOLD),
                ),
                replayed,
                Span::styled(format!("{:>5}ms ", hit.ms), theme.fg(fx::FG)),
                Span::styled(format!(":{:<5} ", hit.port), theme.fg(fx::DIM)),
                Span::styled(hit.path.clone(), theme.fg(fx::FG)),
            ]);
            highlight(line, Some(row.n) == app.traffic.picked)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}
