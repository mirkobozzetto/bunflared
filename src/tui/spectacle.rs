//! The show around the fire while the link loads: meteors crashing into it,
//! a firefighter bunny who tries his best, and the phoenix that brings the
//! link once it is ready.

use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

use super::App;
use super::art::{self, Frame3};
use super::fx::{self, put};

const METEOR_EVERY: (f32, f32) = (0.6, 1.3);
const SHAKE_FOR: Duration = Duration::from_millis(280);
const FIREFIGHTER_FIRST: f32 = 1.0;
const FIREFIGHTER_EVERY: f32 = 7.0;
// Running in, throwing water, running away, in seconds.
const RUN_IN: f32 = 0.6;
const THROW: f32 = 1.4;
const RUN_OFF: f32 = 0.6;
// After the water, the fire comes back taller for a while.
const FLARE_UP: f32 = 1.5;
const RISE: f32 = 1.1;

struct Meteor {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

#[derive(Default)]
pub struct Spectacle {
    meteors: Vec<Meteor>,
    next_meteor: f32,
    shake_until: Option<Instant>,
    /// When the firefighter's current visit started, in seconds since launch.
    firefighter: Option<f32>,
}

impl Spectacle {
    /// How the fire burns at `at` seconds since launch: nearly out while
    /// the water lands, then taller than before. Fuel, and reach.
    pub fn flames(&self, at: f32) -> (f32, f32) {
        let Some(start) = self.firefighter else {
            return (0.9, 0.75);
        };
        let since = at - start - RUN_IN;
        if (0.0..THROW).contains(&since) {
            (0.12, 0.5)
        } else if (THROW..THROW + FLARE_UP).contains(&since) {
            (1.0, 1.05)
        } else {
            (0.9, 0.75)
        }
    }

    pub fn shaking(&self, now: Instant) -> bool {
        self.shake_until.is_some_and(|until| now < until)
    }
}

/// Meteors and the firefighter, over a fire whose top is at `fire_top`.
pub fn loading(app: &mut App, buf: &mut Buffer, area: Rect, fire_top: i32) {
    if app.theme.calm {
        return;
    }
    let (now, e, dt) = (app.now, app.elapsed(), app.dt);
    if e >= app.spectacle.next_meteor {
        let (low, high) = METEOR_EVERY;
        app.spectacle.next_meteor = e + app.rng.range(low, high);
        let x = app
            .rng
            .range(area.width as f32 * 0.25, area.width as f32 + 12.0);
        app.spectacle.meteors.push(Meteor {
            x,
            y: area.y as f32 - 2.0,
            vx: -app.rng.range(14.0, 26.0),
            vy: app.rng.range(14.0, 22.0),
        });
    }
    let impact = fire_top as f32 + 2.0;
    let mut crashed = Vec::new();
    app.spectacle.meteors.retain_mut(|meteor| {
        meteor.x += meteor.vx * dt;
        meteor.y += meteor.vy * dt;
        let landed = meteor.y >= impact;
        if landed {
            crashed.push((meteor.x, meteor.y));
        }
        !landed && meteor.x > -10.0
    });
    for (x, y) in crashed {
        app.particles.blast(&mut app.rng, x, y);
        app.spectacle.shake_until = Some(now + SHAKE_FOR);
    }
    for meteor in &app.spectacle.meteors {
        draw_meteor(buf, app, meteor);
    }
    firefighter(app, buf, area, fire_top, e);
}

fn draw_meteor(buf: &mut Buffer, app: &App, meteor: &Meteor) {
    const TAIL: [&str; 6] = ["▓", "▓", "▒", "▒", "░", "·"];
    let speed = (meteor.vx.powi(2) + meteor.vy.powi(2)).sqrt().max(1.0);
    let (ux, uy) = (-meteor.vx / speed, -meteor.vy / speed);
    for (i, ch) in TAIL.iter().enumerate().rev() {
        let step = (i + 1) as f32 * 1.3;
        let color = fx::hsv(48.0 - i as f32 * 9.0, 1.0, 1.0 - i as f32 * 0.08);
        put(
            buf,
            (meteor.x + ux * step * 2.0).round() as i32,
            (meteor.y + uy * step).round() as i32,
            ch,
            app.theme.fg(color),
        );
    }
    put(
        buf,
        meteor.x.round() as i32,
        meteor.y.round() as i32,
        "●",
        app.theme
            .fg(fx::hsv(55.0, 0.2, 1.0))
            .add_modifier(Modifier::BOLD),
    );
}

/// Runs in with a bucket, throws the water on the fire, runs off before it
/// flares up again.
fn firefighter(app: &mut App, buf: &mut Buffer, area: Rect, fire_top: i32, t: f32) {
    let start = match app.spectacle.firefighter {
        Some(start) if t - start < RUN_IN + THROW + RUN_OFF + FLARE_UP => start,
        _ => {
            let due = app
                .spectacle
                .firefighter
                .map_or(FIREFIGHTER_FIRST, |start| start + FIREFIGHTER_EVERY);
            if t < due {
                return;
            }
            app.spectacle.firefighter = Some(t);
            t
        }
    };
    let since = t - start;
    let spot = area.width as f32 * 0.18;
    let (x, pose, throwing): (f32, &Frame3, bool) = if since < RUN_IN {
        (-10.0 + (spot + 10.0) * since / RUN_IN, run_pose(t), false)
    } else if since < RUN_IN + THROW {
        (spot, &art::FIREFIGHTER_THROW, true)
    } else if since < RUN_IN + THROW + RUN_OFF {
        let away = (since - RUN_IN - THROW) / RUN_OFF;
        (spot - (spot + 12.0) * away, run_pose(t), false)
    } else {
        return;
    };
    let (x, y) = (x.round() as i32, fire_top - 3);
    fx::art(buf, x, y, pose, app.theme.fg(fx::FG));
    let bucket = if throwing { "\\_/" } else { "[_]" };
    put(buf, x + 7, y + 1, bucket, app.theme.fg(fx::RED));
    if throwing {
        for _ in 0..4 {
            app.particles
                .water(&mut app.rng, (x + 10) as f32, (y + 1) as f32);
        }
        let reach = app.rng.range(8.0, 40.0);
        app.particles
            .steam(&mut app.rng, x as f32 + 10.0 + reach, fire_top as f32);
    }
}

fn run_pose(t: f32) -> &'static Frame3 {
    if (t * 14.0) as i32 % 2 == 0 {
        &art::RUN_A
    } else {
        &art::RUN_B
    }
}

/// Where the link box stands at `t` seconds into the launch: carried up
/// from the bottom by the phoenix, then left at `rest`.
pub fn carried(t: f32, rest: i32, bottom: i32) -> i32 {
    let done = (t / RISE).min(1.0);
    let eased = 1.0 - (1.0 - done).powi(3);
    bottom + ((rest - bottom) as f32 * eased).round() as i32
}

/// The phoenix rises out of the dying fire holding the link box, lets go
/// of it, and flies off the top of the screen.
pub fn phoenix(app: &mut App, buf: &mut Buffer, area: Rect, url_y: i32) {
    if app.theme.calm {
        return;
    }
    let t = app.t();
    let (mut x, mut y) = (fx::center(area, 17), url_y - 5);
    if t > RISE {
        let away = t - RISE;
        x += (away * 26.0) as i32;
        y -= (away * 16.0) as i32;
    }
    if y < area.y as i32 - 6 {
        return;
    }
    let wings = if (t * 8.0) as i32 % 2 == 0 {
        &art::PHOENIX_UP
    } else {
        &art::PHOENIX_DOWN
    };
    for (row, line) in wings.iter().enumerate() {
        let hue = 52.0 - row as f32 * 11.0 + (t * 30.0).sin() * 4.0;
        let style = app
            .theme
            .fg(fx::hsv(hue, 0.9, 1.0))
            .add_modifier(Modifier::BOLD);
        for (col, ch) in line.chars().enumerate() {
            if ch != ' ' {
                put(buf, x + col as i32, y + row as i32, &ch.to_string(), style);
            }
        }
    }
    for _ in 0..3 {
        let tail = x as f32 + app.rng.range(6.0, 11.0);
        app.particles.flare(&mut app.rng, tail, (y + 4) as f32);
    }
}
