use ratatui::Frame;
use ratatui::style::Modifier;

use super::{bunny, centered};
use crate::tui::art;
use crate::tui::fx::{self, center};
use crate::tui::{App, GOODBYE};

pub fn goodbye(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    let t = app.t();
    let buf = frame.buffer_mut();
    let theme = &app.theme;
    let cy = area.y as i32 + area.height as i32 / 2;
    let done = (t / GOODBYE).min(1.0);
    centered(
        buf,
        area,
        cy - 5,
        "Closing the rabbit hole...",
        theme.fg(fx::PINK).add_modifier(Modifier::BOLD),
    );
    let pose = if (t * 5.0) as i32 % 2 == 0 {
        &art::WAVE_A
    } else {
        &art::WAVE_B
    };
    bunny(buf, center(area, 10), cy - 3, pose, theme.fg(fx::FG));
    let tunnel = ((area.width as f32 - 10.0) * (1.0 - done)) as usize;
    centered(buf, area, cy + 1, &"═".repeat(tunnel), theme.fg(fx::DIRT));
    if done > 0.4 {
        centered(
            buf,
            area,
            cy + 3,
            "see you later, carrot shipper",
            theme.fg(fx::DIM),
        );
    }
}
