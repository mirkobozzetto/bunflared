use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::panel;
use crate::tui::App;
use crate::tui::fx;

const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub fn draw_stats(app: &App, frame: &mut Frame, area: Rect) {
    let block = panel(app, "stats");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let theme = &app.theme;
    let average = if app.traffic.total == 0 {
        0
    } else {
        app.traffic.ms_sum / app.traffic.total as u64
    };
    let strong = |value: String, color: Color| {
        Span::styled(value, theme.fg(color).add_modifier(Modifier::BOLD))
    };
    let label = |text: &'static str| Span::styled(text, theme.fg(fx::DIM));
    let mut lines = vec![
        Line::from(vec![
            strong(app.traffic.total.to_string(), fx::FG),
            label(" requests  "),
            strong(app.traffic.visitors.len().to_string(), fx::FG),
            label(if app.traffic.visitors.len() == 1 {
                " visitor  "
            } else {
                " visitors  "
            }),
            strong(app.traffic.sockets.to_string(), fx::FG),
            label(" sockets"),
        ]),
        Line::from(vec![
            label("2xx "),
            strong(app.traffic.classes[0].to_string(), fx::GREEN),
            label("  3xx "),
            strong(app.traffic.classes[1].to_string(), fx::CYAN),
            label("  4xx "),
            strong(app.traffic.classes[2].to_string(), fx::YELLOW),
            label("  5xx "),
            strong(app.traffic.classes[3].to_string(), fx::RED),
        ]),
        Line::from(vec![
            label("avg "),
            strong(format!("{average} ms"), fx::FG),
            label("  carrots eaten "),
            strong(app.traffic.classes[0].to_string(), fx::CARROT),
            label("  feedback "),
            strong(app.audience.feedback.to_string(), fx::PINK),
        ]),
    ];
    let width = inner.width as usize;
    let recent: Vec<u64> = app
        .traffic
        .per_second
        .iter()
        .rev()
        .take(width)
        .rev()
        .copied()
        .collect();
    let peak = recent.iter().copied().max().unwrap_or(0).max(1);
    let spark: Vec<Span> = recent
        .iter()
        .map(|&count| {
            let level = if count == 0 {
                0
            } else {
                1 + (count * 6 / peak) as usize
            };
            Span::styled(
                BARS[level.min(7)].to_string(),
                theme.fg(fx::hsv(140.0 - level as f32 * 18.0, 0.7, 1.0)),
            )
        })
        .collect();
    lines.push(Line::from(spark));
    frame.render_widget(Paragraph::new(lines), inner);
}
