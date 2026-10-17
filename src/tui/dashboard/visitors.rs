use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::{focusable, highlight, scroll};
use crate::tui::fx;
use crate::tui::keys::Focus;
use crate::tui::{App, plural};
use crate::visitors::Visitor;

/// "active", "tab hidden", "idle 45s" or "left", with its color.
fn presence(app: &App, visitor: &Visitor) -> (&'static str, String, Color) {
    let away = app.now.saturating_duration_since(visitor.seen);
    let status = visitor.presence.status(away);
    let (glyph, color) = match status.as_str() {
        "left" => ("○", fx::DIM),
        "active" => ("●", fx::GREEN),
        _ => ("◐", fx::YELLOW),
    };
    (glyph, status, color)
}

/// The visitors in the order they are listed: by arrival, those who left last.
pub fn roster(app: &App) -> Vec<&Visitor> {
    let mut visitors = app.audience.roster.by_arrival();
    visitors.sort_by_key(|v| presence(app, v).1 == "left");
    visitors
}

pub fn here(app: &App) -> usize {
    app.audience
        .roster
        .by_arrival()
        .into_iter()
        .filter(|v| presence(app, v).1 != "left")
        .count()
}

pub fn draw_visitors(app: &App, frame: &mut Frame, area: Rect) {
    let block = focusable(
        app,
        &format!("visitors · {} here", here(app)),
        Focus::Visitors,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let theme = &app.theme;
    if app.audience.roster.is_empty() {
        let line = Line::from(Span::styled("Nobody on the page yet.", theme.fg(fx::DIM)));
        frame.render_widget(Paragraph::new(line), inner);
        return;
    }
    let visitors = roster(app);
    let at = visitors
        .iter()
        .position(|v| app.audience.selected.as_ref() == Some(&v.presence.sid));
    let lines: Vec<Line> = visitors
        .into_iter()
        .skip(scroll(at, inner.height as usize))
        .take(inner.height as usize)
        .map(|visitor| {
            let (dot, state, color) = presence(app, visitor);
            let p = &visitor.presence;
            let selected = app.audience.selected.as_ref() == Some(&p.sid);
            let line = Line::from(vec![
                Span::styled(format!("{dot} "), theme.fg(color)),
                Span::styled(format!("{}  ", p.device), theme.fg(fx::FG)),
                Span::styled(format!("{}  ", p.page), theme.fg(fx::CYAN)),
                Span::styled(state, theme.fg(color)),
                Span::styled(
                    format!("  {}", plural(p.clicks as usize, "click")),
                    theme.fg(fx::DIM),
                ),
            ]);
            highlight(line, selected)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}
