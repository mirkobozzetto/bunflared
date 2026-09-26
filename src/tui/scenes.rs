use std::f32::consts::PI;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::art::{self, Frame3};
use super::fx::{self, center, put};
use super::{App, GOODBYE, SNIFF, Theme, spectacle};

const TAGLINE: &str = "share localhost · dig a tunnel · feed the bunny";
const DEPTH_METERS: f32 = 42.0;
const JOKE_EVERY: f32 = 2.4;
const LAUNCH_BURSTS: [f32; 4] = [0.5, 1.1, 1.7, 2.4];

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

/// A fire along the whole bottom of the screen, `share` of its height tall,
/// whose flames climb `reach` of the way up. Returns where it starts.
fn inferno(
    app: &mut App,
    buf: &mut Buffer,
    area: Rect,
    share: f32,
    (fuel, reach): (f32, f32),
) -> i32 {
    let height = (area.height as f32 * share) as usize;
    let top = area.bottom() as i32 - height as i32;
    if app.theme.calm {
        return top;
    }
    let cooling = (2.0 * fx::FIRE_HOT as f32 / (height as f32 * reach))
        .round()
        .max(1.0) as usize;
    app.inferno
        .step(&mut app.rng, area.width as usize, height, fuel, cooling);
    app.inferno.draw(buf, &app.theme, area.x as i32, top);
    top
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
    let flames = app.spectacle.flames(app.elapsed());
    let fire_top = inferno(app, buf, area, 0.5, flames);
    let logo_width = art::big("BUNFLARED")[0].chars().count() as i32;
    let top = area.y as i32 + area.height as i32 / 2 - 7;
    let x0 = center(area, logo_width);
    let reveal = (t / 0.8 * logo_width as f32) as i32;

    if area.width as i32 >= logo_width + 2 {
        // The logo stands in a fire that rises behind and around it.
        if !app.theme.calm {
            let fuel = (t * 1.6).min(0.95);
            let width = (logo_width + 12) as usize;
            app.fire.step(&mut app.rng, width, FIRE_ROWS, fuel, 7);
            app.fire
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
        app.keep_clear = Some(Rect::new(
            (x0 - 1).max(0) as u16,
            top.max(0) as u16,
            logo_width as u16 + 2,
            6,
        ));
        let col = if reveal < logo_width {
            reveal
        } else {
            app.rng.below(logo_width as usize) as i32
        };
        app.particles
            .flare(&mut app.rng, (x0 + col) as f32, top as f32 - 1.0);
        let spark = x0 + app.rng.below(logo_width as usize) as i32;
        app.particles.flare(&mut app.rng, spark as f32, top as f32);
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
    let x = area.x as i32 - 10 + ((area.width as i32 + 20) as f32 * lap) as i32;
    let hop = ((t * 18.0).sin().abs() * 1.5) as i32;
    let pose = if app.theme.calm || (t * 14.0) as i32 % 2 == 0 {
        &art::RUN_A
    } else {
        &art::RUN_B
    };
    bunny(buf, x, top + 9 - hop, pose, app.theme.fg(fx::FG));
    spectacle::loading(app, buf, area, fire_top);
    if !app.theme.calm {
        for _ in 0..2 {
            app.particles
                .flare(&mut app.rng, (x - 1) as f32, (top + 11) as f32);
        }
    }
    hint(buf, area, &app.theme, "any key to skip");
}

pub fn preflight(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    let t = app.t();
    let calm = app.theme.calm;
    let buf = frame.buffer_mut();
    let flames = app.spectacle.flames(app.elapsed());
    let fire_top = inferno(app, buf, area, 0.3, flames);
    spectacle::loading(app, buf, area, fire_top);
    let rows = app.ports.len() as i32;
    let top = area.y as i32 + (area.height as i32 - (3 + rows * 2)) / 2;
    let x0 = center(area, 52);
    centered(
        buf,
        area,
        top,
        "Sniffing your ports",
        app.theme.fg(fx::PINK).add_modifier(Modifier::BOLD),
    );

    for (i, port) in app.ports.iter().enumerate() {
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
    let (t, e, calm) = (app.t(), app.elapsed(), app.theme.calm);
    let buf = frame.buffer_mut();
    let flames = app.spectacle.flames(app.elapsed());
    let fire_top = inferno(app, buf, area, 0.3, flames);
    let width = (area.width as i32 - 8).clamp(24, 72);
    let x0 = center(area, width);
    let top = area.y as i32 + (area.height as i32 - 15).max(0) / 2;
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
        for i in 0..width / 6 {
            let x = x0 + (i * 37) % width;
            let y = top + 3 + (i * 13) % 2;
            let star = if (e * 2.5 + i as f32 * 0.7) as i32 % 3 == 0 {
                "*"
            } else {
                "·"
            };
            put(buf, x, y, star, theme.fg(fx::YELLOW));
        }
    }

    let surface = top + 5;
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
        for row in 1..=4 {
            put(buf, x0 + col, surface + row, "░", theme.fg(fx::DIRT));
        }
    }
    let head = x0 + ((width - 10) as f32 * app.progress) as i32;
    for col in x0..(head + 9).min(x0 + width) {
        for row in 1..=3 {
            put(buf, col, surface + row, " ", Style::new());
        }
    }
    // The tunnel burns behind the bunny, who runs for its life.
    if !calm {
        let length = (head - x0).max(0) as usize;
        app.fire.step(&mut app.rng, length, 3, 0.8, 24);
        app.fire.draw(buf, theme, x0, surface + 1);
    }
    let pose = if calm || (t * 14.0) as i32 % 2 == 0 {
        &art::RUN_A
    } else {
        &art::RUN_B
    };
    bunny(buf, head, surface + 1, pose, theme.fg(fx::FG));

    let bar = width - 24;
    let filled = (bar as f32 * app.progress) as i32;
    let y = surface + 6;
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
        (app.progress * 100.0) as i32,
        app.progress * DEPTH_METERS
    );
    put(buf, x0 + 1 + bar, y, &depth, theme.fg(fx::FG));
    put(buf, x0, y + 1, &app.stage, theme.fg(fx::DIM));
    if let Some(url) = &app.url {
        put(
            buf,
            x0,
            y + 2,
            &format!("reserved {url}"),
            theme.fg(fx::CYAN).add_modifier(Modifier::DIM),
        );
    }

    if !calm {
        app.particles
            .flare(&mut app.rng, (head - 1) as f32, surface as f32);
    }
    spectacle::loading(app, buf, area, fire_top);
}

pub fn launch(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    let (t, e) = (app.t(), app.elapsed());
    let (cx, cy) = (area.width as f32 / 2.0, area.height as f32 / 2.0);
    // The fire dies down: the phoenix is made of it.
    let fuel = (0.9 - t).max(0.0);
    inferno(app, frame.buffer_mut(), area, 0.3, (fuel, 0.75));
    if app.launch_bursts == 0 {
        app.particles.confetti(&mut app.rng, cx, cy, 160);
        app.launch_bursts = 1;
    }
    let next = app.launch_bursts as usize - 1;
    if next < LAUNCH_BURSTS.len() && t >= LAUNCH_BURSTS[next] {
        let x = app.rng.range(10.0, (area.width as f32 - 10.0).max(11.0));
        let y = app.rng.range(3.0, (cy - 2.0).max(4.0));
        app.particles.fireworks(&mut app.rng, x, y);
        app.launch_bursts += 1;
    }

    let buf = frame.buffer_mut();
    let theme = &app.theme;
    let top = cy as i32 - 9;
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
    let hop = ((jump * PI).sin() * 4.0) as i32;
    let pose = if jump < 1.0 {
        &art::HOP
    } else if (t * 4.0) as i32 % 2 == 0 {
        &art::DANCE_A
    } else {
        &art::DANCE_B
    };
    bunny(
        buf,
        center(area, 10),
        cy as i32 - 2 - hop,
        pose,
        theme.fg(fx::FG),
    );

    if let Some(url) = &app.url {
        let inner = url.chars().count() as i32 + 4;
        let x = center(area, inner + 2);
        let y = spectacle::carried(t, cy as i32 + 2, area.bottom() as i32 - 3);
        let border = theme.fg(fx::rainbow(e * 200.0));
        app.keep_clear = Some(Rect::new(
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
        let (note, color) = if app.copied {
            ("✓ copied to your clipboard", fx::GREEN)
        } else {
            ("press c to copy", fx::DIM)
        };
        if y == cy as i32 + 2 {
            centered(buf, area, y + 4, note, theme.fg(color));
        }
        if !app.resolved {
            centered(
                buf,
                area,
                y + 5,
                "DNS is slow today: give the link a minute.",
                theme.fg(fx::YELLOW),
            );
        }
    }
    hint(buf, area, theme, "any key for the dashboard");
    let url_y = spectacle::carried(t, cy as i32 + 2, area.bottom() as i32 - 3);
    spectacle::phoenix(app, buf, area, url_y);
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
    let message = app.failure.as_ref().map_or("", |f| f.message.as_str());
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
    if app.carrots_until.is_some() && app.rng.range(0.0, 1.0) < 0.7 {
        app.particles.carrot(&mut app.rng, area);
    }
    let buf = frame.buffer_mut();
    for runner in &app.runners {
        let body = Rect::new(runner.x.max(0.0) as u16, runner.y.max(0.0) as u16, 9, 3);
        if app.keep_clear.is_some_and(|clear| clear.intersects(body)) {
            continue;
        }
        let pose = if (runner.x / 3.0) as i32 % 2 == 0 {
            &art::RUN_A
        } else {
            &art::RUN_B
        };
        let color = if app.disco {
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
