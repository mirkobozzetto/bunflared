mod boot;
mod digging;
mod failed;
mod goodbye;
mod launch;
mod preflight;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use super::art::{self, Frame3};
use super::fx::{self, center, put};
use super::{App, Theme};

pub use boot::boot;
pub use digging::digging;
pub use failed::failed;
pub use goodbye::goodbye;
pub use launch::launch;
pub use preflight::preflight;

fn bunny(buf: &mut Buffer, x: i32, y: i32, frame: &Frame3, style: Style) {
    fx::art(buf, x, y, frame, style);
}

fn centered(buf: &mut Buffer, area: Rect, y: i32, text: &str, style: Style) {
    put(
        buf,
        center(area, text.chars().count() as i32),
        y,
        text,
        style,
    );
}

fn hint(buf: &mut Buffer, area: Rect, theme: &Theme, text: &str) {
    centered(buf, area, area.bottom() as i32 - 2, text, theme.fg(fx::DIM));
}

#[allow(clippy::too_many_arguments)]
fn big_text(
    buf: &mut Buffer,
    theme: &Theme,
    x: i32,
    y: i32,
    text: &str,
    reveal: i32,
    hue: f32,
    palette: fn(f32) -> Color,
) {
    let cells: Vec<(i32, i32)> = art::big(text)
        .iter()
        .enumerate()
        .flat_map(|(row, line)| {
            line.chars()
                .enumerate()
                .filter(|&(col, ch)| ch != ' ' && col as i32 <= reveal)
                .map(move |(col, _)| (col as i32, row as i32))
        })
        .collect();
    // A dark outline cut into whatever burns behind, so the letters read.
    for &(col, row) in &cells {
        for (dx, dy) in [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ] {
            put(buf, x + col + dx, y + row + dy, " ", Style::new());
        }
    }
    for (col, row) in cells {
        let color = palette(col as f32 * 7.0 + hue);
        put(buf, x + col, y + row, "█", theme.fg(color));
    }
}

pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in text.split_whitespace() {
        let line = lines.last_mut().expect("one line");
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(word.to_string());
        } else {
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
    }
    lines
}

/// Screen-wide mischief: carrot rain and the stampede.
pub fn fun(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    if app.show.carrots_until.is_some() && app.show.rng.range(0.0, 1.0) < 0.7 {
        app.show.particles.carrot(&mut app.show.rng, area);
    }
    let buf = frame.buffer_mut();
    for runner in &app.show.runners {
        let body = Rect::new(runner.x.max(0.0) as u16, runner.y.max(0.0) as u16, 9, 3);
        if app
            .show
            .keep_clear
            .is_some_and(|clear| clear.intersects(body))
        {
            continue;
        }
        let pose = if (runner.x / 3.0) as i32 % 2 == 0 {
            &art::RUN_A
        } else {
            &art::RUN_B
        };
        let color = if app.show.disco {
            fx::rainbow(runner.x * 6.0)
        } else {
            fx::FG
        };
        bunny(
            buf,
            runner.x as i32,
            runner.y as i32,
            pose,
            app.theme.fg(color),
        );
    }
}
