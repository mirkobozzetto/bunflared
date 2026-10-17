use std::f32::consts::PI;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

use super::{big_text, centered, hint};
use crate::tui::art;
use crate::tui::cast;
use crate::tui::fx::{self, center, put};
use crate::tui::{App, spectacle};

const LAUNCH_BURSTS: [f32; 4] = [0.5, 1.1, 1.7, 2.4];

pub fn launch(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    let (t, e) = (app.t(), app.elapsed());
    let (cx, cy) = (area.width as f32 / 2.0, area.height as f32 / 2.0);
    if app.show.launch_bursts == 0 {
        app.show.particles.confetti(&mut app.show.rng, cx, cy, 160);
        app.show.launch_bursts = 1;
    }
    let next = app.show.launch_bursts as usize - 1;
    if next < LAUNCH_BURSTS.len() && t >= LAUNCH_BURSTS[next] {
        let x = app
            .show
            .rng
            .range(10.0, (area.width as f32 - 10.0).max(11.0));
        let y = app.show.rng.range(3.0, (cy - 2.0).max(4.0));
        app.show.particles.fireworks(&mut app.show.rng, x, y);
        app.show.launch_bursts += 1;
    }

    let buf = frame.buffer_mut();
    let theme = &app.theme;
    let top = cy as i32 - 10;
    let live_width = art::big("LIVE!")[0].chars().count() as i32;
    big_text(
        buf,
        theme,
        center(area, live_width),
        top,
        "LIVE!",
        live_width,
        e * 300.0,
        fx::rainbow,
    );

    let jump = (t / 0.9).min(1.0);
    let hop = ((jump * PI).sin() * 2.0) as i32;
    let pose = if jump < 1.0 || (t * 4.0) as i32 % 2 == 0 {
        cast::DANCE_LEFT
    } else {
        cast::DANCE_RIGHT
    };
    let hands = (center(area, cast::width(pose)), cy as i32 - 3 - hop);
    let fg = theme.fg(fx::FG);
    cast::draw(buf, hands, pose, false, |_| fg);

    let Some(url) = &app.boot.url else {
        hint(buf, area, theme, "any key for the dashboard");
        return;
    };
    let inner = url.chars().count() as i32 + 4;
    let rest = center(area, inner + 2);
    let x = spectacle::delivered(t, rest, area.right() as i32);
    let y = cy as i32 + 3;
    let border = theme.fg(fx::rainbow(e * 200.0));
    app.show.keep_clear = Some(Rect::new(
        x.max(0) as u16,
        y.max(0) as u16,
        (inner + 2) as u16,
        3,
    ));
    put(
        buf,
        x,
        y,
        &format!("╭{}╮", "─".repeat(inner as usize)),
        border,
    );
    put(buf, x, y + 1, "│", border);
    put(
        buf,
        x + 3,
        y + 1,
        url,
        theme.fg(fx::CYAN).add_modifier(Modifier::BOLD),
    );
    put(buf, x + inner + 1, y + 1, "│", border);
    put(
        buf,
        x,
        y + 2,
        &format!("╰{}╯", "─".repeat(inner as usize)),
        border,
    );
    let (note, color) = if app.boot.copied {
        ("✓ copied to your clipboard", fx::GREEN)
    } else {
        ("press c to copy", fx::DIM)
    };
    if x == rest {
        centered(buf, area, y + 4, note, theme.fg(color));
    }
    if !app.boot.resolved {
        centered(
            buf,
            area,
            y + 5,
            "DNS is slow today: give the link a minute.",
            theme.fg(fx::YELLOW),
        );
    }
    hint(buf, area, theme, "any key for the dashboard");
    // The dragon stays right of the letters and the bunny.
    let clear = cx as i32 + live_width / 2 + 2;
    let held = (x + inner + 2, y);
    spectacle::delivery(app, buf, area, held, (hands.0 + 6, hands.1 + 1), clear);
}
