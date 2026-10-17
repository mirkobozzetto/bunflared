use ratatui::Frame;
use ratatui::style::Modifier;

use super::centered;
use crate::tui::fx::{self, center, put};
use crate::tui::{App, SNIFF};

pub fn preflight(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    let t = app.t();
    let calm = app.theme.calm;
    let buf = frame.buffer_mut();
    let rows = app.boot.ports.len() as i32;
    let top = area.y as i32 + (area.height as i32 - (3 + rows * 2)) / 2;
    let x0 = center(area, 52);
    centered(
        buf,
        area,
        top,
        "Sniffing your ports",
        app.theme.fg(fx::PINK).add_modifier(Modifier::BOLD),
    );

    for (i, port) in app.boot.ports.iter().enumerate() {
        let appear = 0.3 + i as f32 * SNIFF;
        if !calm && t < appear {
            continue;
        }
        let y = top + 2 + i as i32 * 2;
        let sniffing = port.checked.is_none() || (!calm && t < appear + SNIFF);
        let (face, status, color) = match port.checked {
            Some(true) if !sniffing => ("(^.^)", "✓ alive".to_string(), fx::GREEN),
            Some(false) if !sniffing => ("(;_;)", "✖ nobody home".to_string(), fx::RED),
            _ => {
                let face = if (t * 8.0) as i32 % 2 == 0 {
                    "(•.•)"
                } else {
                    "(•o•)"
                };
                (
                    face,
                    format!("sniff{}", ".".repeat((t * 6.0) as usize % 4)),
                    fx::DIM,
                )
            }
        };
        put(buf, x0, y, face, app.theme.fg(fx::FG));
        let target = format!("{:<13} localhost:{}", port.route, port.port);
        put(buf, x0 + 7, y, &target, app.theme.fg(fx::CYAN));
        put(buf, x0 + 38, y, &status, app.theme.fg(color));
    }
}
