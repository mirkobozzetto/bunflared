use ratatui::Frame;
use ratatui::style::Modifier;

use super::{bunny, centered, hint, wrap};
use crate::tui::App;
use crate::tui::art;
use crate::tui::fx::{self, center};

pub fn failed(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    let buf = frame.buffer_mut();
    let theme = &app.theme;
    let cy = area.y as i32 + area.height as i32 / 2;
    bunny(buf, center(area, 10), cy - 6, &art::SAD, theme.fg(fx::FG));
    centered(
        buf,
        area,
        cy - 2,
        "Oh no.",
        theme.fg(fx::RED).add_modifier(Modifier::BOLD),
    );
    let message = app.boot.failure.as_ref().map_or("", |f| f.message.as_str());
    let width = (area.width as usize).saturating_sub(6).max(20);
    for (i, line) in message
        .lines()
        .flat_map(|line| wrap(line, width))
        .enumerate()
    {
        centered(buf, area, cy + i as i32, &line, theme.fg(fx::FG));
    }
    hint(buf, area, theme, "any key to exit");
}
