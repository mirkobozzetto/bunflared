use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::centered;
use crate::tui::art;
use crate::tui::cast;
use crate::tui::fx::{self, center, put};
use crate::tui::{App, spectacle};

const DEPTH_METERS: f32 = 42.0;
const JOKE_EVERY: f32 = 2.4;

// Title, jokes, the dragon's sky, the labels, the burrow, the bar.
const DIGGING_ROWS: i32 = 24;
// Extra rows the soaked dragon may zigzag through, on a tall terminal.
const ZIGZAG_ROWS: i32 = 4;

pub fn digging(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    let (t, calm) = (app.t(), app.theme.calm);
    let buf = frame.buffer_mut();
    let width = (area.width as i32 - 8).clamp(24, 72);
    let x0 = center(area, width);
    let room = (area.height as i32 - DIGGING_ROWS).clamp(0, ZIGZAG_ROWS);
    let top = area.y as i32 + (area.height as i32 - DIGGING_ROWS - room).max(0) / 2;
    // Sparks and steam stay off the title and the jokes.
    app.show.keep_clear = Some(Rect::new(area.x, top.max(0) as u16, area.width, 2));
    let theme = &app.theme;

    centered(
        buf,
        area,
        top,
        "Digging a rabbit hole to the internet",
        theme.fg(fx::PINK).add_modifier(Modifier::BOLD),
    );
    if !calm {
        let joke = art::JOKES[(t / JOKE_EVERY) as usize % art::JOKES.len()];
        let typed: String = joke
            .chars()
            .take(((t % JOKE_EVERY) * 45.0) as usize)
            .collect();
        put(
            buf,
            center(area, joke.chars().count() as i32),
            top + 1,
            &typed,
            theme.fg(fx::DIM),
        );
    }

    // Under the dragon's sky, eleven lines tall, and the row of the labels.
    // A terminal too short for it keeps the burrow without the show.
    let roomy = area.height as i32 >= DIGGING_ROWS;
    let surface = top + if roomy { 14 + room } else { 3 };
    put(buf, x0 - 11, surface - 1, "your machine", theme.fg(fx::DIM));
    put(
        buf,
        x0 + width - 3,
        surface - 1,
        "the internet",
        theme.fg(fx::DIM),
    );
    put(buf, x0 - 2, surface, "⌂", theme.fg(fx::CARROT));
    put(buf, x0 + width + 1, surface, "☁", theme.fg(fx::FG));
    for col in 0..width {
        let grass = ["w", "W", "v", "w", "V", "w"][(col * 7 % 6) as usize];
        put(buf, x0 + col, surface, grass, theme.fg(fx::GRASS));
        for row in 1..=5 {
            put(buf, x0 + col, surface + row, "░", theme.fg(fx::DIRT));
        }
    }
    let run = cast::width(cast::RUN_A);
    let head = x0 + ((width - run - 1) as f32 * app.boot.progress) as i32;
    for col in x0..(head + run).min(x0 + width) {
        for row in 1..=4 {
            put(buf, col, surface + row, " ", Style::new());
        }
    }
    let pose = if calm || (t * 14.0) as i32 % 2 == 0 {
        cast::RUN_A
    } else {
        cast::RUN_B
    };
    let fg = theme.fg(fx::FG);
    let x_pose = head + run - cast::width(pose);
    cast::draw(buf, (x_pose, surface + 1), pose, false, |_| fg);

    let bar = width - 24;
    let filled = (bar as f32 * app.boot.progress) as i32;
    let y = surface + 7;
    put(buf, x0, y, "[", theme.fg(fx::DIM));
    for i in 0..bar {
        let (ch, color) = if i < filled {
            ("█", fx::hsv(20.0 + i as f32 * 100.0 / bar as f32, 0.7, 1.0))
        } else {
            ("░", fx::DIM)
        };
        put(buf, x0 + 1 + i, y, ch, theme.fg(color));
    }
    let depth = format!(
        "] {:>3}%  depth {:.1} m",
        (app.boot.progress * 100.0) as i32,
        app.boot.progress * DEPTH_METERS
    );
    put(buf, x0 + 1 + bar, y, &depth, theme.fg(fx::FG));
    put(buf, x0, y + 1, &app.boot.stage, theme.fg(fx::DIM));
    if let Some(url) = &app.boot.url {
        put(
            buf,
            x0,
            y + 2,
            &format!("reserved {url}"),
            theme.fg(fx::CYAN).add_modifier(Modifier::DIM),
        );
    }
    if roomy {
        spectacle::burrow(app, buf, area, (x0, width), (top + 2, surface));
    }
}
