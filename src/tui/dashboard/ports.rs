use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::panel;
use crate::tui::App;
use crate::tui::fx;

const ECG: [char; 16] = [
    '▁', '▁', '▁', '▂', '▁', '▁', '▇', '▃', '▁', '▁', '▁', '▁', '▁', '▁', '▁', '▁',
];

pub fn draw_ports(app: &App, frame: &mut Frame, area: Rect) {
    let block = panel(app, "ports");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let e = app.elapsed();
    let shift = if app.theme.calm {
        0
    } else {
        (e * 8.0) as usize
    };
    let lines: Vec<Line> = app
        .boot
        .ports
        .iter()
        .enumerate()
        .map(|(i, port)| {
            let target = format!(" {:<12} :{:<5} ", port.route, port.port);
            if port.up {
                let beat = !app.theme.calm && (e * 1.3 + i as f32 * 0.37).fract() < 0.25;
                let heart = if beat {
                    app.theme.fg(fx::RED).add_modifier(Modifier::BOLD)
                } else {
                    app.theme.fg(fx::PINK)
                };
                let ecg: String = (0..10)
                    .map(|j| ECG[(j + shift + i * 5) % ECG.len()])
                    .collect();
                Line::from(vec![
                    Span::styled("♥", heart),
                    Span::styled(target, app.theme.fg(fx::CYAN)),
                    Span::styled(ecg, app.theme.fg(fx::GREEN)),
                    Span::styled(format!(" {}", port.hits), app.theme.fg(fx::DIM)),
                ])
            } else {
                Line::from(vec![
                    Span::styled("✖", app.theme.fg(fx::RED).add_modifier(Modifier::BOLD)),
                    Span::styled(target, app.theme.fg(fx::CYAN)),
                    Span::styled("▁".repeat(10), app.theme.fg(fx::RED)),
                    Span::styled(" down", app.theme.fg(fx::RED).add_modifier(Modifier::BOLD)),
                ])
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}
