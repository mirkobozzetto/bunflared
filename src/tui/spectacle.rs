//! The show over the rabbit hole while the link loads: a dragon flies to and
//! fro above the burrow, spitting fire on it, and a firefighter bunny puts
//! out every flame with his water cannon. Then the phoenix brings the link.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

use super::App;
use super::art;
use super::cast;
use super::fx::{self, Particle, put};

const DRAGON: Color = Color::Rgb(215, 50, 60);
const STEEL: Color = Color::Rgb(175, 190, 205);
// The dragon's band: eleven lines, then the row of the labels.
const SKY: i32 = 12;
const FLY_SPEED: f32 = 11.0;
const FLAP: f32 = 0.22;
const SPIT_EVERY: f32 = 1.3;
const SPIT_FLIGHT: f32 = 0.4;
const MAX_FLAMES: usize = 3;
const FLAME_WIDTH: usize = 4;
const FLAME_ROWS: usize = 3;
// Seconds of water a flame takes to go out, the trip included.
const DOUSE: f32 = 0.9;
const JET_FLIGHT: f32 = 0.45;
const JET_GRAVITY: f32 = 26.0;
const RISE: f32 = 1.1;

struct Spit {
    from: (f32, f32),
    to: (f32, f32),
    at: f32,
}

struct Flame {
    x: i32,
    fire: fx::Fire,
    wet: f32,
}

#[derive(Default)]
pub struct Spectacle {
    /// The dragon's column and heading, 1 to the right, -1 to the left.
    dragon: Option<(f32, f32)>,
    next_spit: f32,
    spits: Vec<Spit>,
    flames: Vec<Flame>,
}

/// The dragon and the firefighter over a burrow `width` long from `x0`,
/// whose grass is on row `surface`.
pub fn burrow(app: &mut App, buf: &mut Buffer, area: Rect, (x0, width): (i32, i32), surface: i32) {
    if app.theme.calm {
        return;
    }
    let (e, dt) = (app.elapsed(), app.dt);
    let fighter = (x0 + 2, surface - 5);
    let reach = fighter.0 + cast::width(cast::FIREFIGHTER);
    let wide = cast::width(cast::DRAGON_UP);
    let (low, high) = (reach + 2, (x0 + width + 4).min(area.right() as i32) - wide);
    let land = ((reach + 3) as f32, (x0 + width - 6) as f32);
    let sky = surface - SKY;
    let show = &mut app.spectacle;

    let (mut x, mut heading) = show.dragon.unwrap_or((high as f32, -1.0));
    x = (x + heading * FLY_SPEED * dt).clamp(low as f32, high.max(low) as f32);
    if x <= low as f32 {
        heading = 1.0;
    } else if x >= high as f32 {
        heading = -1.0;
    }
    show.dragon = Some((x, heading));
    let (grid, mouth) = if (e / FLAP) as i32 % 2 == 0 {
        (cast::DRAGON_UP, cast::DRAGON_UP_MOUTH)
    } else {
        (cast::DRAGON_DOWN, cast::DRAGON_DOWN_MOUTH)
    };
    let (flip, left) = (heading < 0.0, x.round() as i32);
    let mouth = (
        if flip {
            left + wide - 1 - mouth.0
        } else {
            left + mouth.0
        } as f32,
        (sky + mouth.1) as f32,
    );

    if e >= show.next_spit && show.flames.len() + show.spits.len() < MAX_FLAMES {
        show.next_spit = e + SPIT_EVERY;
        let ahead = app.rng.range(3.0, 10.0) * heading;
        let to = (
            (mouth.0 + ahead).clamp(land.0, land.1),
            (surface - 1) as f32,
        );
        show.spits.push(Spit {
            from: mouth,
            to,
            at: e,
        });
    }
    let mut landed = Vec::new();
    show.spits.retain(|spit| {
        let done = e - spit.at >= SPIT_FLIGHT;
        if done {
            landed.push(spit.to.0.round() as i32);
        }
        !done
    });
    show.flames.extend(landed.into_iter().map(|x| Flame {
        x,
        fire: fx::Fire::default(),
        wet: 0.0,
    }));

    // The firefighter always aims at the oldest flame.
    let nozzle = (reach as f32, (surface - 2) as f32);
    if let Some(flame) = show.flames.first_mut() {
        flame.wet += dt;
        for _ in 0..2 {
            let flight = JET_FLIGHT + app.rng.range(-0.05, 0.05);
            let (dx, dy) = (flame.x as f32 - nozzle.0, (surface - 1) as f32 - nozzle.1);
            app.particles.0.push(Particle {
                x: nozzle.0,
                y: nozzle.1,
                vx: dx / flight + app.rng.range(-1.5, 1.5),
                vy: dy / flight - JET_GRAVITY * flight / 2.0,
                gravity: JET_GRAVITY,
                life: flight,
                ch: app.rng.pick("·°,'~"),
                color: fx::hsv(app.rng.range(190.0, 215.0), 0.6, 1.0),
                leaf: false,
            });
        }
    }
    if show.flames.first().is_some_and(|flame| flame.wet >= DOUSE) {
        let out = show.flames.remove(0);
        for _ in 0..10 {
            let x = out.x as f32 + app.rng.range(-1.5, 1.5);
            app.particles.steam(&mut app.rng, x, (surface - 1) as f32);
        }
    }

    let theme = &app.theme;
    for flame in &mut show.flames {
        let fuel = 0.95 * (1.0 - flame.wet / DOUSE);
        flame
            .fire
            .step(&mut app.rng, FLAME_WIDTH, FLAME_ROWS, fuel, 14);
        let at = flame.x - FLAME_WIDTH as i32 / 2;
        flame
            .fire
            .draw(buf, theme, at, surface + 1 - FLAME_ROWS as i32);
    }
    let red = theme.fg(DRAGON);
    cast::draw(buf, (left, sky), grid, flip, |_| red);
    cast::draw(buf, fighter, cast::FIREFIGHTER, false, |ink| match ink {
        b'r' => theme.fg(fx::RED),
        b's' => theme.fg(STEEL),
        _ => theme.fg(fx::FG),
    });
    let ball = theme
        .fg(fx::hsv(30.0, 0.9, 1.0))
        .add_modifier(Modifier::BOLD);
    for spit in &show.spits {
        let k = (e - spit.at) / SPIT_FLIGHT;
        let (x, y) = (
            spit.from.0 + (spit.to.0 - spit.from.0) * k,
            spit.from.1 + (spit.to.1 - spit.from.1) * k,
        );
        put(buf, x.round() as i32, y.round() as i32, "●", ball);
        app.particles.flare(&mut app.rng, x, y);
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
