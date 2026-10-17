//! The show over the rabbit hole while the link loads: a dragon flies to and
//! fro above the burrow, spitting fire on it, and a firefighter bunny puts
//! out every flame with his water cannon. Then the dragon flies the link in,
//! and a carrot from the bunny calms it down.

use std::f32::consts::PI;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

use super::App;
use super::fx::{self, Particle, put};
use super::{cast, hose};

const DRAGON: Color = Color::Rgb(215, 50, 60);
const STEEL: Color = Color::Rgb(175, 190, 205);
const DRAGON_ROWS: i32 = 11;
const FLY_SPEED: f32 = 11.0;
const FLAP: f32 = 0.22;
const SPIT_EVERY: f32 = 1.3;
const SPIT_FLIGHT: f32 = 0.4;
const MAX_FLAMES: usize = 3;
const FLAME_WIDTH: usize = 4;
const FLAME_ROWS: usize = 3;
// Seconds of water a flame takes to go out.
const DOUSE: f32 = 0.9;
// Seconds of water on the dragon before it is as wild as it gets.
const SOAKED: f32 = 6.0;
const DRIP: Color = Color::Rgb(200, 240, 255);
/// Seconds the dragon takes to flee once the link is ready, before LIVE!.
pub const FLEE: f32 = 1.0;
// The launch, in seconds: the box flies in, the carrot flies to the dragon,
// the dragon munches, then flies off.
const ARRIVE: f32 = 1.2;
const THROW_AT: f32 = 1.3;
const THROW: f32 = 0.5;
const LEAVE_AT: f32 = 2.6;
// Wings beat slower once the dragon has eaten.
const CALM_FLAP: f32 = 0.5;

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
    /// Seconds of water the dragon took: the more, the wilder it flies.
    soaked: f32,
    munched: bool,
}

/// The dragon's pose at `t` with its top left cell at `at`, and where its
/// mouth is on screen.
fn dragon(t: f32, flap: f32, at: (i32, i32), flip: bool) -> (&'static str, (i32, i32)) {
    let (grid, mouth) = if (t / flap) as i32 % 2 == 0 {
        (cast::DRAGON_UP, cast::DRAGON_UP_MOUTH)
    } else {
        (cast::DRAGON_DOWN, cast::DRAGON_DOWN_MOUTH)
    };
    let x = if flip {
        cast::width(grid) - 1 - mouth.0
    } else {
        mouth.0
    };
    (grid, (at.0 + x, at.1 + mouth.1))
}

/// The dragon and the firefighter over a burrow `width` long from `x0`:
/// the dragon flies between row `sky` and the labels above the grass, on
/// row `surface`.
pub fn burrow(
    app: &mut App,
    buf: &mut Buffer,
    area: Rect,
    (x0, width): (i32, i32),
    (sky, surface): (i32, i32),
) {
    if app.theme.calm {
        return;
    }
    let (e, dt) = (app.elapsed(), app.dt);
    let fleeing = app.ready_at.map(|at| (app.now - at).as_secs_f32());
    let fighter = (x0 + 2, surface - 5);
    let nozzle = (
        fighter.0 + cast::FIREFIGHTER_NOZZLE.0 + 1,
        fighter.1 + cast::FIREFIGHTER_NOZZLE.1,
    );
    let reach = fighter.0 + cast::width(cast::FIREFIGHTER);
    let wide = cast::width(cast::DRAGON_UP);
    let (low, high) = (reach + 2, (x0 + width + 4).min(area.right() as i32) - wide);
    let land = ((reach + 3) as f32, (x0 + width - 6) as f32);
    // Rows the dragon may zigzag through, when the terminal is tall enough.
    let room = surface - 1 - sky - DRAGON_ROWS;
    let show = &mut app.spectacle;
    let wild = (show.soaked / SOAKED).min(1.0);

    let (mut x, mut heading) = show.dragon.unwrap_or((high as f32, -1.0));
    if app.rng.range(0.0, 1.0) < wild * dt * 1.5 {
        heading = -heading;
    }
    x = (x + heading * FLY_SPEED * (1.0 + wild) * dt).clamp(low as f32, high.max(low) as f32);
    if x <= low as f32 {
        heading = 1.0;
    } else if x >= high as f32 {
        heading = -1.0;
    }
    show.dragon = Some((x, heading));
    let bob = ((e * 6.0).sin() * wild * room as f32 / 2.0).round() as i32;
    let mut at = (x.round() as i32, sky + room / 2 + bob);
    // It flees off to the right, under the title and the jokes.
    if let Some(gone) = fleeing {
        at.0 += (gone * 60.0) as i32;
        at.1 = (at.1 - (gone * 6.0) as i32).max(sky);
    }
    let flip = heading < 0.0 && fleeing.is_none();
    let (grid, mouth) = dragon(e, FLAP * (1.0 - 0.6 * wild), at, flip);
    let mouth = (mouth.0 as f32, mouth.1 as f32);
    let seen = at.1 + DRAGON_ROWS > area.y as i32 && at.0 < area.right() as i32;

    // The wetter it gets, the faster it spits, and the worse it aims.
    if fleeing.is_none() && e >= show.next_spit && show.flames.len() + show.spits.len() < MAX_FLAMES
    {
        show.next_spit = e + SPIT_EVERY * (1.0 - 0.5 * wild);
        let ahead = app.rng.range(3.0, 10.0) * heading + app.rng.range(-10.0, 10.0) * wild;
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

    // The firefighter puts out the oldest flame; with none, he hoses the
    // dragon.
    let body = (at.0 + wide / 2, at.1 + DRAGON_ROWS / 2);
    let target = match show.flames.first_mut() {
        Some(flame) => {
            flame.wet += dt;
            Some((flame.x, surface - 1))
        }
        None if seen && fleeing.is_none() => {
            show.soaked += dt;
            Some(body)
        }
        None => None,
    };
    if show.flames.first().is_some_and(|flame| flame.wet >= DOUSE) {
        let out = show.flames.remove(0);
        for _ in 0..10 {
            let x = out.x as f32 + app.rng.range(-1.5, 1.5);
            app.particles.steam(&mut app.rng, x, (surface - 1) as f32);
        }
    }
    if seen && app.rng.range(0.0, 1.0) < wild * 0.5 {
        // A soaked dragon drips, and steams where the water hits it.
        let x = (at.0 + 3) as f32 + app.rng.range(0.0, (wide - 6) as f32);
        let y = (at.1 + 3) as f32 + app.rng.range(0.0, 6.0);
        app.particles.0.push(Particle {
            x,
            y,
            vx: app.rng.range(-1.0, 1.0),
            vy: 0.0,
            gravity: 18.0,
            life: app.rng.range(0.4, 0.8),
            ch: app.rng.pick("·',"),
            color: DRIP,
            leaf: false,
        });
        if target == Some(body) {
            app.particles
                .steam(&mut app.rng, body.0 as f32, body.1 as f32);
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
    if seen {
        cast::draw(buf, at, grid, flip, |_| red);
    }
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
    if let Some(to) = target {
        hose::spray(app, buf, (nozzle, to), sky);
    }
}

/// Where the link box's left edge is at `t` seconds into the launch: flown
/// in from `right` by the dragon, then left at `rest`.
pub fn delivered(t: f32, rest: i32, right: i32) -> i32 {
    let eased = 1.0 - (1.0 - (t / ARRIVE).min(1.0)).powi(3);
    right + ((rest - right) as f32 * eased).round() as i32
}

/// The dragon holds the link box by its right end at `held`, never left of
/// `clear`. The bunny throws a carrot from `hands`; the dragon eats it,
/// calms down and flies off.
pub fn delivery(
    app: &mut App,
    buf: &mut Buffer,
    area: Rect,
    held: (i32, i32),
    hands: (i32, i32),
    clear: i32,
) {
    if app.theme.calm {
        return;
    }
    let t = app.t();
    let mut at = ((held.0 - 12).max(clear), held.1 - 11);
    let leaving = t > LEAVE_AT;
    if leaving {
        let away = t - LEAVE_AT;
        at.0 += (away * 30.0) as i32;
        at.1 -= (away * 14.0) as i32;
    }
    if at.1 + 11 < area.y as i32 || at.0 > area.right() as i32 {
        return;
    }
    let calm = t > THROW_AT + THROW;
    let flap = if calm && !leaving { CALM_FLAP } else { FLAP };
    // It faces the bunny until it flies off.
    let (grid, mouth) = dragon(t, flap, at, !leaving);
    let red = app.theme.fg(DRAGON);
    cast::draw(buf, at, grid, !leaving, |_| red);

    let k = (t - THROW_AT) / THROW;
    if (0.0..1.0).contains(&k) {
        let x = hands.0 as f32 + (mouth.0 - hands.0) as f32 * k;
        let y = hands.1 as f32 + (mouth.1 - hands.1) as f32 * k - (k * PI).sin() * 3.0;
        let (carrot, leaf) = (app.theme.fg(fx::CARROT), app.theme.fg(fx::LEAF));
        let at = (x.round() as i32 - 3, y.round() as i32 - 1);
        cast::draw(buf, at, cast::CARROT, false, |ink| {
            if ink == b'l' { leaf } else { carrot }
        });
    } else if k >= 1.0 && !app.spectacle.munched {
        app.spectacle.munched = true;
        let (x, y) = (mouth.0 as f32, mouth.1 as f32);
        for _ in 0..10 {
            app.particles.0.push(Particle {
                x,
                y,
                vx: app.rng.range(-6.0, 6.0),
                vy: app.rng.range(-4.0, 1.0),
                gravity: 15.0,
                life: app.rng.range(0.4, 0.8),
                ch: app.rng.pick("·,'"),
                color: fx::CARROT,
                leaf: false,
            });
        }
        app.particles.hearts(&mut app.rng, x, y);
    }
}
