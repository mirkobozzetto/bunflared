use ratatui::Frame;
use ratatui::style::Modifier;

use super::{bunny, centered};
use crate::clock::uptime;
use crate::notes;
use crate::tui::art;
use crate::tui::fx::{self, center};
use crate::tui::{App, GOODBYE, plural};

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

/// What stays in the scrollback once the terminal is restored.
pub fn recap(app: &App) {
    if let Some(failure) = &app.boot.failure {
        eprintln!("bunflared: {}", failure.message);
        return;
    }
    let Some(url) = &app
        .boot
        .url
        .as_ref()
        .filter(|_| app.boot.ready_at.is_some())
    else {
        return;
    };
    let paint = |code: &str, text: &str| {
        if app.theme.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    };
    let badges: Vec<String> = app
        .show
        .unlocked
        .iter()
        .map(|a| format!("★ {}", a.name))
        .collect();
    println!();
    println!(
        "  {}      {}",
        paint("35", r" (\(\ "),
        paint("1", "bunflared · session recap")
    );
    println!(
        "  {}     {}  {}",
        paint("35", r" ( ^.^)/"),
        paint("36", url),
        paint("2", "(closed)")
    );
    println!(
        "  {}    {} live · {} · {} · {}",
        paint("35", r#" o_(")(")"#),
        uptime(app.live_for()),
        plural(app.traffic.total as usize, "request"),
        plural(app.traffic.visitors.len(), "visitor"),
        plural(app.traffic.classes[3] as usize, "error"),
    );
    if !badges.is_empty() {
        println!("               {}", paint("33", &badges.join("  ")));
    }
    if app.audience.feedback > 0 {
        let note = format!(
            "✎ {} saved in {}/",
            plural(app.audience.feedback as usize, "feedback note"),
            notes::FOLDER
        );
        println!("               {}", paint("36", &note));
    }
    println!();
}
