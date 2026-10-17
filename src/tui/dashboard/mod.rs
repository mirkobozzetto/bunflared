mod chat;
mod details;
mod lane;
mod overlays;
mod ports;
mod requests;
mod stats;
mod visitors;

use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

use super::boot::Qr;
use super::fx::{self, put};
use super::keys::Focus;
use super::{App, Theme, plural};
use crate::clock::uptime;
use chat::{draw_side, split_side};
use lane::draw_lane;
use ports::draw_ports;
use requests::draw_log;
use stats::draw_stats;
use visitors::{draw_visitors, here};

pub use overlays::overlays;
pub use visitors::roster;

const MIN_WIDTH: u16 = 64;
const MIN_HEIGHT: u16 = 20;
const QR_MIN_MAIN: u16 = 56;

const KEYS: [(&str, &str); 8] = [
    ("c", "copy"),
    ("o", "open"),
    ("r", "qr"),
    ("m", "message"),
    ("g", "go"),
    ("x", "clear"),
    ("?", "help"),
    ("q", "quit"),
];

fn hms(secs: u64) -> String {
    format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

/// " · 12m 04s left" while `--for` runs, nothing otherwise.
fn left(app: &App) -> String {
    let Some(at) = app.boot.record.as_ref().and_then(|r| r.expires_at) else {
        return String::new();
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format!(" · {} left", uptime(at.saturating_sub(now)))
}

/// A panel whose border shows it has the keyboard focus.
fn focusable<'a>(app: &App, title: &str, focus: Focus) -> Block<'a> {
    let block = panel(app, title);
    if app.keys.focus == Some(focus) {
        block.border_style(app.theme.fg(fx::PINK))
    } else {
        block
    }
}

/// The first line to draw so that the selected one stays in view.
fn scroll(selected: Option<usize>, height: usize) -> usize {
    selected.map_or(0, |i| (i + 1).saturating_sub(height))
}

fn highlight(line: Line<'_>, selected: bool) -> Line<'_> {
    if selected {
        line.style(Style::new().add_modifier(Modifier::REVERSED))
    } else {
        line
    }
}

fn panel<'a>(app: &App, title: &str) -> Block<'a> {
    let border = if app.show.disco {
        fx::rainbow(app.elapsed() * 150.0 + title.len() as f32 * 45.0)
    } else {
        fx::DIM
    };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(app.theme.fg(border))
        .title(Span::styled(
            format!(" {title} "),
            app.theme.fg(fx::PINK).add_modifier(Modifier::BOLD),
        ))
}

pub fn render(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        return compact(app, frame, area);
    }
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    draw_header(app, frame, header);

    let qr_size = app.boot.qr_code.as_ref().map_or(0, |qr| qr.size as u16);
    let (qr_width, qr_height) = (qr_size + 2, qr_size.div_ceil(2) + 2);
    let fits = qr_size > 0 && body.width >= qr_width + QR_MIN_MAIN && body.height >= qr_height;
    let mut visitors = None;
    let main = if fits {
        let [main, side] =
            Layout::horizontal([Constraint::Min(0), Constraint::Length(qr_width)]).areas(body);
        let [side, below] =
            Layout::vertical([Constraint::Length(qr_height), Constraint::Min(0)]).areas(side);
        visitors = (below.height >= 4).then_some(below);
        let block = panel(app, "scan me");
        let inner = block.inner(side);
        frame.render_widget(block, side);
        app.show.spots.qr = Some(side);
        if let Some(qr) = &app.boot.qr_code {
            draw_qr(
                frame.buffer_mut(),
                &app.theme,
                qr,
                inner.x as i32,
                inner.y as i32,
            );
        }
        app.show.keep_clear = Some(inner);
        main
    } else {
        body
    };

    let [lane, middle, log] = Layout::vertical([
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Min(3),
    ])
    .areas(main);
    let (ports, stats) = if visitors.is_none() {
        // No room under the QR code: the visitors take a third of the middle row.
        let [ports, stats, right] = Layout::horizontal([Constraint::Ratio(1, 3); 3]).areas(middle);
        visitors = Some(right);
        (ports, stats)
    } else {
        let [ports, stats] =
            Layout::horizontal([Constraint::Percentage(48), Constraint::Percentage(52)])
                .areas(middle);
        (ports, stats)
    };
    draw_lane(app, frame, lane);
    draw_ports(app, frame, ports);
    draw_stats(app, frame, stats);
    if let Some(area) = visitors {
        draw_visitors(app, frame, area);
    }
    app.show.spots.visitors = visitors;
    let (log, side) = split_side(app, log);
    app.show.spots.log = Some(log);
    draw_log(app, frame, log);
    if let Some(side) = side {
        draw_side(app, frame, side);
    }
    draw_footer(app, frame, footer);
}

fn draw_header(app: &App, frame: &mut Frame, area: Rect) {
    let e = app.elapsed();
    let title: Vec<Span> = " bunflared "
        .chars()
        .enumerate()
        .map(|(i, ch)| {
            let color = fx::rainbow(i as f32 * 30.0 + e * 90.0);
            Span::styled(
                ch.to_string(),
                app.theme.fg(color).add_modifier(Modifier::BOLD),
            )
        })
        .collect();
    let dot = if app.theme.calm || (e * 2.0) as i32 % 2 == 0 {
        fx::RED
    } else {
        fx::PINK
    };
    let live = Line::from(vec![
        Span::styled(" ●", app.theme.fg(dot)),
        Span::styled(
            format!(" LIVE {}{} ", hms(app.live_for()), left(app)),
            app.theme.fg(fx::FG).add_modifier(Modifier::BOLD),
        ),
    ])
    .right_aligned();
    let border = if app.show.disco {
        fx::rainbow(e * 150.0)
    } else {
        fx::DIM
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(app.theme.fg(border))
        .title(Line::from(title))
        .title_top(live);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let url = app.boot.url.clone().unwrap_or_default();
    let (note, color) = if app.boot.copied {
        ("✓ in your clipboard", fx::GREEN)
    } else {
        ("press c to copy", fx::DIM)
    };
    let line = Line::from(vec![
        Span::styled(
            url,
            app.theme
                .fg(fx::CYAN)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ),
        Span::raw("   "),
        Span::styled(note, app.theme.fg(color)),
    ]);
    frame.render_widget(Paragraph::new(line), inner);
}

fn status_color(status: u16) -> Color {
    match status {
        0..300 => fx::GREEN,
        300..400 => fx::CYAN,
        400..500 => fx::YELLOW,
        _ => fx::RED,
    }
}

fn draw_footer(app: &App, frame: &mut Frame, area: Rect) {
    let mut spans = vec![Span::raw(" ")];
    for (key, label) in KEYS {
        spans.push(Span::styled(
            key,
            app.theme.fg(fx::PINK).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(format!(" {label}   "), app.theme.fg(fx::DIM)));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn compact(app: &App, frame: &mut Frame, area: Rect) {
    let theme = &app.theme;
    let status = if app.boot.ready_at.is_some() {
        format!("● LIVE {}{}", hms(app.live_for()), left(app))
    } else {
        "digging...".into()
    };
    let lines = vec![
        Line::from(vec![
            Span::styled(
                "bunflared ",
                theme.fg(fx::PINK).add_modifier(Modifier::BOLD),
            ),
            Span::styled(status, theme.fg(fx::RED)),
        ]),
        Line::from(Span::styled(
            app.boot.url.clone().unwrap_or_default(),
            theme.fg(fx::CYAN).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!(
                "{} here now · {} feedback",
                here(app),
                app.audience.feedback
            ),
            theme.fg(fx::FG),
        )),
        Line::from(Span::styled(
            format!(
                "{} · {} · {}",
                plural(app.traffic.total as usize, "request"),
                plural(app.traffic.visitors.len(), "visitor"),
                plural(app.traffic.classes[3] as usize, "error")
            ),
            theme.fg(fx::FG),
        )),
        Line::from(Span::styled(
            "c copy · o open · r qr · m message · q quit",
            theme.fg(fx::DIM),
        )),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}

/// Half blocks with explicit black and white, so phones scan it on any theme.
fn draw_qr(buf: &mut Buffer, theme: &Theme, qr: &Qr, x: i32, y: i32) {
    const BLACK: Color = Color::Rgb(0, 0, 0);
    const WHITE: Color = Color::Rgb(255, 255, 255);
    let dark = |col: usize, row: usize| row < qr.size && qr.dark[row * qr.size + col];
    for row in 0..qr.size.div_ceil(2) {
        for col in 0..qr.size {
            let (top, bottom) = (dark(col, row * 2), dark(col, row * 2 + 1));
            let (cx, cy) = (x + col as i32, y + row as i32);
            if theme.color {
                let fg = if top { BLACK } else { WHITE };
                let bg = if bottom { BLACK } else { WHITE };
                put(buf, cx, cy, "▀", Style::new().fg(fg).bg(bg));
            } else {
                // ponytail: assumes a dark terminal, light modules are drawn.
                let ch = match (top, bottom) {
                    (false, false) => "█",
                    (false, true) => "▀",
                    (true, false) => "▄",
                    (true, true) => " ",
                };
                put(buf, cx, cy, ch, Style::new());
            }
        }
    }
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}
