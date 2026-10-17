use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use super::details::draw_details;
use super::{centered_rect, draw_qr, highlight, panel};
use crate::tui::App;
use crate::tui::fx;
use crate::tui::keys::Ask;

const HELP: [&str; 20] = [
    "c    copy the link",
    "o    open it in your browser",
    "r    big QR code for phones",
    "f    fireworks",
    "",
    "↑ ↓  pick a visitor",
    "tab  switch to the requests",
    "     enter: its details, p: replay it",
    "/    filter the requests",
    "esc  pick nobody: everyone again",
    "m    message them",
    "g    send them to a page",
    "R    reload their page",
    "e    send them a reaction",
    "x    clear the chat",
    "",
    "?    this help",
    "q    stop sharing",
    "psst: there are secrets.",
    "the bunny knows a few codes.",
];
const HELP_WIDTH: u16 = 40;

/// The text box of `g`, low on the screen so the panels stay readable.
fn draw_prompt(app: &App, frame: &mut Frame, area: Rect) {
    let Some(prompt) = &app.keys.prompt else {
        return;
    };
    let suggestions = app.audience.suggestions(prompt);
    let (title, hint) = match prompt.ask {
        Ask::Go => (
            format!("send {} to", app.audience.target()),
            " enter send · tab pick · esc cancel ",
        ),
        Ask::Chat => (
            format!("message to {}", app.audience.target()),
            " enter send · esc close ",
        ),
        Ask::React => (
            format!("react to {}", app.audience.target()),
            " 1-4 or tab, enter send · esc cancel ",
        ),
        Ask::Filter => (
            "requests with".to_string(),
            " method, status or path · enter filter · esc clear ",
        ),
    };
    let typed = prompt.ask != Ask::React;
    let width = 60.min(area.width.saturating_sub(2));
    let height = 2 + u16::from(typed) + suggestions.len() as u16;
    let x = area.x + (area.width - width) / 2;
    let y = area.bottom().saturating_sub(height + 2).max(area.y);
    let rect = Rect::new(x, y, width, height.min(area.height));
    frame.render_widget(Clear, rect);
    let block = panel(app, &title)
        .border_style(app.theme.fg(fx::PINK))
        .title_bottom(Line::from(hint).right_aligned());
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let theme = &app.theme;
    let mut lines = Vec::new();
    if typed {
        lines.push(Line::from(vec![
            Span::styled("› ", theme.fg(fx::PINK).add_modifier(Modifier::BOLD)),
            Span::styled(prompt.text.clone(), theme.fg(fx::FG)),
            Span::styled("▏", theme.fg(fx::PINK)),
        ]));
    }
    lines.extend(suggestions.into_iter().enumerate().map(|(i, page)| {
        let line = Line::from(Span::styled(format!("  {page}"), theme.fg(fx::CYAN)));
        highlight(line, prompt.pick == Some(i))
    }));
    frame.render_widget(Paragraph::new(lines), inner);
}

pub fn overlays(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    if app.keys.qr
        && let Some(qr) = &app.boot.qr_code
    {
        let rect = centered_rect(area, qr.size as u16 + 4, qr.size.div_ceil(2) as u16 + 4);
        frame.render_widget(Clear, rect);
        let block = panel(app, "scan me").title_bottom(Line::from(" esc to close ").centered());
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        draw_qr(
            frame.buffer_mut(),
            &app.theme,
            qr,
            inner.x as i32 + 1,
            inner.y as i32 + 1,
        );
        app.show.keep_clear = Some(rect);
    }
    if app.keys.help {
        let rect = centered_rect(area, HELP_WIDTH, HELP.len() as u16 + 4);
        frame.render_widget(Clear, rect);
        let block = panel(app, "keys");
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        let lines: Vec<Line> = HELP
            .iter()
            .map(|line| {
                let color = if line.starts_with("psst") || line.starts_with("the bunny") {
                    fx::YELLOW
                } else {
                    fx::FG
                };
                Line::from(Span::styled(format!(" {line}"), app.theme.fg(color)))
            })
            .collect();
        frame.render_widget(
            Paragraph::new(lines),
            inner.inner(ratatui::layout::Margin::new(0, 1)),
        );
    }
    draw_details(app, frame, area);
    draw_prompt(app, frame, area);
    for (i, toast) in app.show.toasts.iter().enumerate() {
        let width = (Span::raw(toast.title.as_str())
            .width()
            .max(Span::raw(toast.body.as_str()).width()) as u16
            + 4)
        .min(area.width);
        let age = (app.now - toast.born).as_secs_f32();
        let slide = if app.theme.calm {
            0
        } else {
            ((1.0 - (age / 0.25).min(1.0)) * width as f32) as u16
        };
        let x = area.right().saturating_sub(width + 1) + slide;
        // Stacked up from the footer, clear of the QR code at the top right.
        let Some(y) = area.bottom().checked_sub(2 + (i as u16 + 1) * 4) else {
            continue;
        };
        if y < area.y || x >= area.right() {
            continue;
        }
        let rect = Rect::new(x, y, width.min(area.right() - x), 4);
        frame.render_widget(Clear, rect);
        let border = if toast.gold { fx::GOLD } else { fx::CYAN };
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(app.theme.fg(border));
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        let lines = vec![
            Line::from(Span::styled(
                toast.title.clone(),
                app.theme.fg(border).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(toast.body.clone(), app.theme.fg(fx::FG))),
        ];
        frame.render_widget(Paragraph::new(lines), inner);
    }
}
