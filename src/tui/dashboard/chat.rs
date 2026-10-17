use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::panel;
use crate::tui::App;
use crate::tui::fx::{self, put};
use crate::tui::scenes::wrap;

/// The radar of the selected visitor and the chat, once someone has talked,
/// take the right of the request log.
pub fn split_side(app: &App, log: Rect) -> (Rect, Option<Rect>) {
    if app.audience.chat.is_empty() && app.audience.selected.is_none() {
        return (log, None);
    }
    let [log, side] =
        Layout::horizontal([Constraint::Min(0), Constraint::Percentage(42)]).areas(log);
    (log, Some(side))
}

pub fn draw_side(app: &App, frame: &mut Frame, area: Rect) {
    match (
        app.audience.selected.is_some(),
        app.audience.chat.is_empty(),
    ) {
        (true, false) => {
            let [radar, chat] =
                Layout::vertical([Constraint::Percentage(50), Constraint::Min(0)]).areas(area);
            draw_radar(app, frame, radar);
            draw_chat(app, frame, chat);
        }
        (true, true) => draw_radar(app, frame, area),
        _ => draw_chat(app, frame, area),
    }
}

/// The selected visitor's viewport, scaled, with their pointer in it.
fn draw_radar(app: &App, frame: &mut Frame, area: Rect) {
    let visitor = app
        .audience
        .selected
        .as_ref()
        .and_then(|sid| app.audience.roster.get(sid));
    let device = visitor.map_or("nobody", |v| v.presence.device.as_str());
    let block = panel(app, &format!("radar · {device}"));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let theme = &app.theme;
    let pointer = visitor.and_then(|v| app.audience.pointers.get(&v.presence.sid));
    let Some((pointer, at)) = pointer else {
        let line = Line::from(Span::styled(
            "Their pointer shows here once it moves.",
            theme.fg(fx::DIM),
        ));
        frame.render_widget(Paragraph::new(line), inner);
        return;
    };
    let still = (app.now - *at).as_secs();
    let caption = match still {
        0 => format!("{}×{} · moving", pointer.w, pointer.h),
        secs => format!("{}×{} · still for {secs}s", pointer.w, pointer.h),
    };
    let [room, bottom] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(caption, theme.fg(fx::DIM))).centered()),
        bottom,
    );
    if room.width < 4 || room.height < 3 {
        return;
    }
    // A cell is about twice as tall as it is wide.
    let ratio = pointer.w.max(1.0) / pointer.h.max(1.0);
    let (mut width, mut height) = (room.height as f32 * 2.0 * ratio, room.height as f32);
    if width > room.width as f32 {
        (width, height) = (room.width as f32, room.width as f32 / 2.0 / ratio);
    }
    let width = (width as u16).clamp(4, room.width);
    let height = (height as u16).clamp(3, room.height);
    let screen = Rect::new(room.x + (room.width - width) / 2, room.y, width, height);
    frame.render_widget(Block::bordered().border_style(theme.fg(fx::DIM)), screen);
    let inside = screen.inner(Margin::new(1, 1));
    let spot = |value: f32, size: f32, start: u16, cells: u16| {
        start as i32
            + ((value / size.max(1.0)).clamp(0.0, 1.0) * cells.saturating_sub(1) as f32).round()
                as i32
    };
    let x = spot(pointer.x, pointer.w, inside.x, inside.width);
    let y = spot(pointer.y, pointer.h, inside.y, inside.height);
    let glyph = if still == 0 { "◉" } else { "●" };
    put(
        frame.buffer_mut(),
        x,
        y,
        glyph,
        theme.fg(fx::PINK).add_modifier(Modifier::BOLD),
    );
}

fn draw_chat(app: &App, frame: &mut Frame, area: Rect) {
    let block = panel(app, "chat");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let theme = &app.theme;
    let width = inner.width.saturating_sub(2).max(8) as usize;
    let mut lines = Vec::new();
    for message in &app.audience.chat {
        let (who, color) = if message.mine {
            (format!("you → {}", message.who), fx::PINK)
        } else {
            (message.who.clone(), fx::CYAN)
        };
        let mut head = vec![
            Span::styled(format!("{} ", message.clock), theme.fg(fx::DIM)),
            Span::styled(who, theme.fg(color).add_modifier(Modifier::BOLD)),
        ];
        if !message.page.is_empty() {
            head.push(Span::styled(
                format!(" {}", message.page),
                theme.fg(fx::DIM),
            ));
        }
        lines.push(Line::from(head));
        for text in message.text.lines().flat_map(|line| wrap(line, width)) {
            lines.push(Line::from(Span::styled(
                format!("  {text}"),
                theme.fg(fx::FG),
            )));
        }
    }
    let newest = lines.split_off(lines.len().saturating_sub(inner.height as usize));
    frame.render_widget(Paragraph::new(newest), inner);
}
