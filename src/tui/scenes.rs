use std::f32::consts::PI;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::art::{self, Frame3};
use super::cast;
use super::fx::{self, center, put};
use super::{App, GOODBYE, SNIFF, Theme, spectacle};

const TAGLINE: &str = "share localhost · dig a tunnel · feed the bunny";
const DEPTH_METERS: f32 = 42.0;
const JOKE_EVERY: f32 = 2.4;
const LAUNCH_BURSTS: [f32; 4] = [0.5, 1.1, 1.7, 2.4];
// Title, jokes, the dragon's sky, the labels, the burrow, the bar.
const DIGGING_ROWS: i32 = 24;
// Extra rows the soaked dragon may zigzag through, on a tall terminal.
const ZIGZAG_ROWS: i32 = 4;

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

// Two laps across the screen during the boot.
const DASH_LAP: f32 = 0.75;
const FIRE_ROWS: usize = 12;

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

pub fn boot(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    let t = app.t();
    let buf = frame.buffer_mut();
    let logo_width = art::big("BUNFLARED")[0].chars().count() as i32;
    let top = area.y as i32 + area.height as i32 / 2 - 7;
    let x0 = center(area, logo_width);
    let reveal = (t / 0.8 * logo_width as f32) as i32;

    if area.width as i32 >= logo_width + 2 {
        // The logo stands in a fire that rises behind and around it.
        if !app.theme.calm {
            let fuel = (t * 1.6).min(0.95);
            let width = (logo_width + 12) as usize;
            app.show
                .fire
                .step(&mut app.show.rng, width, FIRE_ROWS, fuel, 7);
            app.show
                .fire
                .draw(buf, &app.theme, x0 - 6, top + 6 - FIRE_ROWS as i32);
        }
        big_text(
            buf,
            &app.theme,
            x0,
            top,
            "BUNFLARED",
            reveal,
            t * 120.0,
            fx::blaze,
        );
        app.show.keep_clear = Some(Rect::new(
            (x0 - 1).max(0) as u16,
            top.max(0) as u16,
            logo_width as u16 + 2,
            6,
        ));
        let col = if reveal < logo_width {
            reveal
        } else {
            app.show.rng.below(logo_width as usize) as i32
        };
        app.show
            .particles
            .flare(&mut app.show.rng, (x0 + col) as f32, top as f32 - 1.0);
        let spark = x0 + app.show.rng.below(logo_width as usize) as i32;
        app.show
            .particles
            .flare(&mut app.show.rng, spark as f32, top as f32);
    } else {
        centered(
            buf,
            area,
            top + 2,
            "bunflared",
            app.theme.fg(fx::PINK).add_modifier(Modifier::BOLD),
        );
    }

    let typed = ((t - 0.6).max(0.0) * 70.0) as usize;
    let tagline: String = TAGLINE.chars().take(typed).collect();
    put(
        buf,
        center(area, TAGLINE.chars().count() as i32),
        top + 6,
        &tagline,
        app.theme.fg(fx::CYAN),
    );

    // The bunny dashes across, again and again, trailing sparks.
    let lap = (t % DASH_LAP) / DASH_LAP;
    let run = cast::width(cast::RUN_A);
    let x = area.x as i32 - run + ((area.width as i32 + run) as f32 * lap) as i32;
    let hop = ((t * 18.0).sin().abs() * 1.5) as i32;
    let pose = if app.theme.calm || (t * 14.0) as i32 % 2 == 0 {
        cast::RUN_A
    } else {
        cast::RUN_B
    };
    // Both poses keep the head at the same place.
    let x_pose = x + run - cast::width(pose);
    let fg = app.theme.fg(fx::FG);
    cast::draw(buf, (x_pose, top + 8 - hop), pose, false, |_| fg);
    if !app.theme.calm {
        for _ in 0..2 {
            app.show
                .particles
                .flare(&mut app.show.rng, (x - 1) as f32, (top + 11) as f32);
        }
    }
    hint(buf, area, &app.theme, "any key to skip");
}

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
