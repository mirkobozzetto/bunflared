use ratatui::Frame;
use ratatui::layout::{Margin, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use super::{centered_rect, panel, status_color};
use crate::event::{CAPTURE, Capture};
use crate::tui::fx;
use crate::tui::traffic::{Replay, Row};
use crate::tui::{App, Theme};

const BODY_LINES: usize = 400;

fn kib(bytes: usize) -> String {
    format!("{:.1} KiB", bytes as f32 / 1024.0)
}

/// The start of a body as text, or what it is when it is not text.
fn body_lines(theme: &Theme, headers: &hyper::HeaderMap, capture: &Capture) -> Vec<Line<'static>> {
    let kind = headers
        .get(hyper::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let text = capture.readable(headers);
    let note = |text: String| Line::from(Span::styled(format!("  {text}"), theme.fg(fx::DIM)));
    if capture.total == 0 {
        return vec![note("empty".into())];
    }
    if !text {
        let what = if kind.is_empty() { "binary" } else { kind };
        return vec![note(format!("{what}, {}, not shown", kib(capture.total)))];
    }
    let mut lines: Vec<Line> = String::from_utf8_lossy(&capture.bytes)
        .lines()
        .take(BODY_LINES)
        .map(|line| Line::from(Span::styled(format!("  {line}"), theme.fg(fx::FG))))
        .collect();
    if !capture.complete() {
        lines.push(note(format!(
            "… the first {} KiB of {} are kept",
            CAPTURE / 1024,
            kib(capture.total)
        )));
    }
    lines
}

fn header_lines(theme: &Theme, headers: &hyper::HeaderMap) -> Vec<Line<'static>> {
    headers
        .iter()
        .map(|(name, value)| {
            Line::from(vec![
                Span::styled(format!("  {name}: "), theme.fg(fx::CYAN)),
                Span::styled(
                    String::from_utf8_lossy(value.as_bytes()).into_owned(),
                    theme.fg(fx::FG),
                ),
            ])
        })
        .collect()
}

fn details_lines(app: &App, row: &Row) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let hit = &row.hit;
    let exchange = &hit.exchange;
    let section = |title: &str| {
        Line::from(Span::styled(
            title.to_string(),
            theme.fg(fx::PINK).add_modifier(Modifier::BOLD),
        ))
    };
    let mut lines = vec![Line::from(vec![
        Span::styled(format!("{} ", hit.method), theme.fg(fx::PURPLE)),
        Span::styled(format!("{}  ", hit.path), theme.fg(fx::FG)),
        Span::styled(
            hit.status.to_string(),
            theme
                .fg(status_color(hit.status))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                " · {} ms · :{} · {} · from {}",
                hit.ms, hit.port, row.clock, hit.visitor
            ),
            theme.fg(fx::DIM),
        ),
    ])];
    let replay = match row.replayed {
        Some(Replay::Done { status, ms }) => format!("replayed: {status} in {ms} ms"),
        Some(Replay::Pending) => "replaying…".into(),
        Some(Replay::Failed) => "the replay failed".into(),
        None => hit
            .unreplayable()
            .unwrap_or_else(|| "p replays it to your local server".into()),
    };
    lines.push(Line::from(Span::styled(
        format!("↻ {replay}"),
        theme.fg(fx::DIM),
    )));
    lines.push(Line::raw(""));
    lines.push(section("Request headers"));
    lines.extend(header_lines(theme, &exchange.request_headers));
    lines.push(section("Request body"));
    let request = exchange.request_body.lock().unwrap();
    lines.extend(body_lines(theme, &exchange.request_headers, &request));
    lines.push(Line::raw(""));
    lines.push(section("Response headers"));
    lines.extend(header_lines(theme, &exchange.response_headers));
    lines.push(section("Response body"));
    let response = exchange.response_body.lock().unwrap();
    lines.extend(body_lines(theme, &exchange.response_headers, &response));
    lines
}

/// Enter on a request: everything that went through, scrollable.
pub fn draw_details(app: &mut App, frame: &mut Frame, area: Rect) {
    let Some(row) = app
        .traffic
        .details
        .and_then(|n| app.traffic.rows.iter().find(|row| row.n == n))
    else {
        return;
    };
    let lines = details_lines(app, row);
    let rect = centered_rect(
        area,
        area.width.saturating_sub(8).min(110),
        area.height.saturating_sub(4),
    );
    frame.render_widget(Clear, rect);
    let block = panel(app, "request")
        .border_style(app.theme.fg(fx::PINK))
        .title_bottom(Line::from(" ↑ ↓ scroll · p replay · c curl · esc close ").right_aligned());
    let inner = block.inner(rect).inner(Margin::new(1, 0));
    frame.render_widget(block, rect);
    let most = lines.len().saturating_sub(inner.height as usize) as u16;
    app.traffic.details_scroll = app.traffic.details_scroll.min(most);
    frame.render_widget(
        Paragraph::new(lines).scroll((app.traffic.details_scroll, 0)),
        inner,
    );
    app.show.keep_clear = Some(rect);
}
