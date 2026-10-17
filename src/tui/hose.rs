//! The firefighter's water jet: a thick stream that waves as it aims,
//! throwing drops all around and splashing where it lands.

use std::f32::consts::PI;

use ratatui::buffer::Buffer;
use ratatui::style::Color;

use super::App;
use super::cast;
use super::fx::Particle;

const WATER: Color = Color::Rgb(139, 233, 253);
const DROP: Color = Color::Rgb(200, 240, 255);
const DROPS: usize = 3;
const SPLASHES: usize = 2;

/// Sprays from the nozzle at `from` to `to`, both in cells, arcing no
/// higher than row `ceiling`.
pub fn spray(app: &mut App, buf: &mut Buffer, (from, to): ((i32, i32), (i32, i32)), ceiling: i32) {
    let e = app.elapsed();
    // In dots: two across a cell, four down.
    let start = (from.0 as f32 * 2.0, from.1 as f32 * 4.0 + 1.5);
    let end = (
        to.0 as f32 * 2.0 + (e * 5.0).sin() * 2.0,
        to.1 as f32 * 4.0 + 2.0,
    );
    // Two dots of margin for the jet's thickness and its wave.
    let headroom = (start.1.min(end.1) - ceiling as f32 * 4.0 - 2.0).max(0.0);
    let arc = ((end.0 - start.0).hypot(end.1 - start.1) * 0.2)
        .max(4.0)
        .min(headroom);
    let point = |s: f32| {
        let wave = (s * 9.0 - e * 16.0).sin() * 0.8 * s;
        (
            start.0 + (end.0 - start.0) * s,
            start.1 + (end.1 - start.1) * s - (s * PI).sin() * arc + wave,
        )
    };

    let steps = ((end.0 - start.0).abs() + (end.1 - start.1).abs()) as usize * 2 + 2;
    let mut dots = Vec::with_capacity(steps * 3);
    for i in 0..=steps {
        let s = i as f32 / steps as f32;
        let (x, y) = point(s);
        let thick = if s < 0.5 { 3 } else { 2 };
        for t in 0..thick {
            dots.push((x.round() as i32, y.round() as i32 - 1 + t));
        }
    }
    let left = dots.iter().map(|d| d.0).min().unwrap_or(0).div_euclid(2) * 2;
    let top = dots.iter().map(|d| d.1).min().unwrap_or(0).div_euclid(4) * 4;
    let right = dots.iter().map(|d| d.0).max().unwrap_or(0);
    let bottom = dots.iter().map(|d| d.1).max().unwrap_or(0);
    let mut rows = vec![vec![b'.'; (right - left + 1) as usize]; (bottom - top + 1) as usize];
    for (x, y) in dots {
        rows[(y - top) as usize][(x - left) as usize] = b'#';
    }
    let grid: Vec<String> = rows
        .into_iter()
        .map(|row| String::from_utf8(row).unwrap_or_default())
        .collect();
    let water = app.theme.fg(WATER);
    cast::draw(buf, (left / 2, top / 4), &grid.join("\n"), false, |_| water);

    let drop = |(x, y): (f32, f32), (vx, vy): (f32, f32), app: &mut App| {
        let life = app.show.rng.range(0.3, 0.9);
        let ch = app.show.rng.pick("·'°,");
        app.show.particles.0.push(Particle {
            x: x / 2.0,
            y: y / 4.0,
            vx,
            vy,
            gravity: 20.0,
            life,
            ch,
            color: DROP,
            leaf: false,
        });
    };
    for _ in 0..DROPS {
        let at = point(app.show.rng.range(0.15, 1.0));
        let speed = (
            app.show.rng.range(-6.0, 6.0),
            app.show.rng.range(-6.0, -1.0),
        );
        drop(at, speed, app);
    }
    for _ in 0..SPLASHES {
        let speed = (
            app.show.rng.range(-10.0, 10.0),
            app.show.rng.range(-9.0, -3.0),
        );
        drop(point(1.0), speed, app);
    }
}
