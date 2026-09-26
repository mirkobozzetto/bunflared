use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::Theme;

pub const PINK: Color = Color::Rgb(255, 121, 198);
pub const CARROT: Color = Color::Rgb(255, 170, 80);
pub const GREEN: Color = Color::Rgb(80, 250, 123);
pub const CYAN: Color = Color::Rgb(139, 233, 253);
pub const YELLOW: Color = Color::Rgb(241, 250, 140);
pub const RED: Color = Color::Rgb(255, 85, 85);
pub const PURPLE: Color = Color::Rgb(189, 147, 249);
pub const DIM: Color = Color::Rgb(110, 118, 160);
pub const FG: Color = Color::Rgb(240, 240, 235);
pub const GOLD: Color = Color::Rgb(255, 215, 90);
pub const DIRT: Color = Color::Rgb(120, 78, 42);
pub const GRASS: Color = Color::Rgb(90, 200, 100);
pub const LEAF: Color = Color::Rgb(60, 190, 80);

/// xorshift: plenty for confetti, no dependency.
pub struct Rng(u64);

impl Rng {
    pub fn seeded() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64);
        Self(nanos | 1)
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    pub fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (self.next() % 10_000) as f32 / 10_000.0 * (high - low)
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    pub fn pick(&mut self, text: &str) -> char {
        let chars: Vec<char> = text.chars().collect();
        chars[self.below(chars.len())]
    }
}

pub fn hsv(hue: f32, saturation: f32, value: f32) -> Color {
    let h = hue.rem_euclid(360.0) / 60.0;
    let c = value * saturation;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = value - c;
    let byte = |v: f32| ((v + m) * 255.0) as u8;
    Color::Rgb(byte(r), byte(g), byte(b))
}

/// The palette is drawn for dark backgrounds; this deepens a color so it
/// reads on a light one. Near-white text turns near-black.
pub fn for_light(color: Color) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return color;
    };
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let delta = max - r.min(g).min(b);
    let saturation = if max == 0.0 { 0.0 } else { delta / max };
    if saturation < 0.15 {
        let gray = ((1.0 - max) * 255.0) as u8;
        return Color::Rgb(gray, gray, gray);
    }
    let hue = if max == r {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    // Yellow stays pale at any brightness on white: pull it toward amber.
    let (hue, value) = if (35.0..80.0).contains(&hue) {
        (hue.min(48.0) - 6.0, 0.48)
    } else {
        (hue, max * 0.6)
    };
    hsv(hue, (saturation * 1.3 + 0.1).min(1.0), value)
}

pub fn rainbow(hue: f32) -> Color {
    hsv(hue, 0.65, 1.0)
}

/// White-hot yellows, for letters standing in the fire.
pub fn blaze(hue: f32) -> Color {
    hsv(40.0 + hue.rem_euclid(30.0) / 2.0, 0.3, 1.0)
}

pub const FIRE_HOT: u8 = 36;

/// A small heat simulation: the bottom row burns, the heat climbs a row per
/// frame, drifting sideways and cooling as it goes. That is how flames flicker.
#[derive(Default)]
pub struct Fire {
    width: usize,
    heat: Vec<u8>,
}

impl Fire {
    /// `fuel` is the share of the bottom row alight, `cooling` how fast the
    /// flames die down on their way up: the higher, the shorter.
    pub fn step(&mut self, rng: &mut Rng, width: usize, height: usize, fuel: f32, cooling: usize) {
        if width == 0 || height == 0 {
            return;
        }
        if self.width != width || self.heat.len() != width * height {
            self.width = width;
            self.heat = vec![0; width * height];
            // Start ablaze rather than from cold embers.
            for _ in 0..height {
                self.burn(rng, height, fuel, cooling);
            }
        }
        self.burn(rng, height, fuel, cooling);
    }

    fn burn(&mut self, rng: &mut Rng, height: usize, fuel: f32, cooling: usize) {
        let width = self.width;
        let bottom = (height - 1) * width;
        for x in 0..width {
            self.heat[bottom + x] = if rng.range(0.0, 1.0) < fuel {
                FIRE_HOT - rng.below(6) as u8
            } else {
                rng.below(8) as u8
            };
        }
        for y in 0..height - 1 {
            for x in 0..width {
                let below = self.heat[(y + 1) * width + x];
                let cool = rng.below(cooling + 1) as u8;
                let drift = (x + rng.below(3)).saturating_sub(1).min(width - 1);
                self.heat[y * width + drift] = below.saturating_sub(cool);
            }
        }
    }

    pub fn draw(&self, buf: &mut Buffer, theme: &Theme, x: i32, y: i32) {
        for (i, &heat) in self.heat.iter().enumerate() {
            if heat > 0 {
                let (ch, color) = flame(heat);
                let (col, row) = ((i % self.width) as i32, (i / self.width) as i32);
                put(buf, x + col, y + row, ch, theme.fg(color));
            }
        }
    }
}

/// Embers are sparse and deep red, the core is a white-hot yellow.
fn flame(heat: u8) -> (&'static str, Color) {
    let t = heat as f32 / FIRE_HOT as f32;
    let ch = match t {
        ..0.12 => "·",
        ..0.28 => "░",
        ..0.45 => "▒",
        ..0.65 => "▓",
        _ => "█",
    };
    (ch, hsv(t * 50.0, 1.0 - t * t * 0.35, 0.6 + t * 0.4))
}

/// Moves the whole frame `dx` cells sideways, for a jolt.
pub fn shake(buf: &mut Buffer, dx: i32) {
    let area = *buf.area();
    for y in area.top()..area.bottom() {
        let row: Vec<_> = (area.left()..area.right())
            .map(|x| buf[(x, y)].clone())
            .collect();
        for (i, x) in (area.left()..area.right()).enumerate() {
            let from = i as i32 - dx;
            buf[(x, y)] = usize::try_from(from)
                .ok()
                .and_then(|from| row.get(from).cloned())
                .unwrap_or_default();
        }
    }
}

/// Writes `text` at (x, y), clipped to the buffer; never panics off-screen.
pub fn put(buf: &mut Buffer, x: i32, y: i32, text: &str, style: Style) {
    let area = *buf.area();
    if y < area.top() as i32 || y >= area.bottom() as i32 {
        return;
    }
    for (i, ch) in text.chars().enumerate() {
        let cx = x + i as i32;
        if cx < area.left() as i32 || cx >= area.right() as i32 {
            continue;
        }
        if let Some(cell) = buf.cell_mut((cx as u16, y as u16)) {
            cell.set_char(ch).set_style(style);
        }
    }
}

/// Draws multi-line art, spaces transparent.
pub fn art(buf: &mut Buffer, x: i32, y: i32, lines: &[&str], style: Style) {
    for (row, line) in lines.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            if ch != ' ' {
                put(buf, x + col as i32, y + row as i32, &ch.to_string(), style);
            }
        }
    }
}

pub fn center(area: Rect, w: i32) -> i32 {
    area.x as i32 + (area.width as i32 - w) / 2
}

pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub gravity: f32,
    pub life: f32,
    pub ch: char,
    pub color: Color,
    pub leaf: bool,
}

#[derive(Default)]
pub struct Particles(pub Vec<Particle>);

impl Particles {
    pub fn step(&mut self, dt: f32) {
        for p in &mut self.0 {
            p.vy += p.gravity * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.life -= dt;
        }
        self.0.retain(|p| p.life > 0.0 && p.y < 500.0);
    }

    pub fn draw(&self, buf: &mut Buffer, theme: &Theme, avoid: Option<Rect>) {
        for p in &self.0 {
            let (x, y) = (p.x.round() as i32, p.y.round() as i32);
            let inside = |rect: Rect| {
                x >= rect.left() as i32
                    && x < rect.right() as i32
                    && y >= rect.top() as i32 - 1
                    && y < rect.bottom() as i32
            };
            if avoid.is_some_and(inside) {
                continue;
            }
            let mut style = theme.fg(p.color);
            if p.life < 0.3 {
                style = style.add_modifier(Modifier::DIM);
            }
            put(buf, x, y, &p.ch.to_string(), style);
            if p.leaf {
                put(buf, x, y - 1, "ψ", theme.fg(LEAF));
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn fireworks(&mut self, rng: &mut Rng, x: f32, y: f32) {
        let hue = rng.range(0.0, 360.0);
        for _ in 0..48 {
            let angle = rng.range(0.0, std::f32::consts::TAU);
            let speed = rng.range(3.0, 11.0);
            self.0.push(Particle {
                x,
                y,
                vx: angle.cos() * speed * 2.0,
                vy: angle.sin() * speed,
                gravity: 7.0,
                life: rng.range(0.7, 1.6),
                ch: rng.pick("*+·•o"),
                color: hsv(hue + rng.range(-35.0, 35.0), 0.7, 1.0),
                leaf: false,
            });
        }
    }

    pub fn confetti(&mut self, rng: &mut Rng, x: f32, y: f32, count: usize) {
        for _ in 0..count {
            self.0.push(Particle {
                x,
                y,
                vx: rng.range(-28.0, 28.0),
                vy: rng.range(-20.0, -5.0),
                gravity: 15.0,
                life: rng.range(1.4, 2.6),
                ch: rng.pick("▪*+~o•▫"),
                color: rainbow(rng.range(0.0, 360.0)),
                leaf: false,
            });
        }
    }

    pub fn hearts(&mut self, rng: &mut Rng, x: f32, y: f32) {
        for _ in 0..16 {
            self.0.push(Particle {
                x: x + rng.range(-8.0, 8.0),
                y: y + rng.range(0.0, 3.0),
                vx: rng.range(-2.5, 2.5),
                vy: rng.range(-9.0, -4.0),
                gravity: 0.0,
                life: rng.range(1.2, 2.4),
                ch: if rng.below(3) == 0 { '♡' } else { '♥' },
                color: hsv(rng.range(330.0, 360.0), 0.6, 1.0),
                leaf: false,
            });
        }
    }

    /// A meteor hitting the fire.
    pub fn blast(&mut self, rng: &mut Rng, x: f32, y: f32) {
        for _ in 0..56 {
            let angle = rng.range(std::f32::consts::PI, std::f32::consts::TAU);
            let speed = rng.range(6.0, 16.0);
            self.0.push(Particle {
                x,
                y,
                vx: angle.cos() * speed * 2.2,
                vy: angle.sin() * speed,
                gravity: 14.0,
                life: rng.range(0.5, 1.3),
                ch: rng.pick("*+•·▪"),
                color: hsv(rng.range(0.0, 55.0), 0.9, 1.0),
                leaf: false,
            });
        }
    }

    pub fn water(&mut self, rng: &mut Rng, x: f32, y: f32) {
        self.0.push(Particle {
            x,
            y,
            vx: rng.range(12.0, 30.0),
            vy: rng.range(-9.0, -2.0),
            gravity: 26.0,
            life: rng.range(0.6, 1.1),
            ch: rng.pick("·°,'~"),
            color: hsv(rng.range(190.0, 215.0), 0.6, 1.0),
            leaf: false,
        });
    }

    pub fn steam(&mut self, rng: &mut Rng, x: f32, y: f32) {
        self.0.push(Particle {
            x,
            y,
            vx: rng.range(-1.5, 1.5),
            vy: rng.range(-4.5, -2.0),
            gravity: -0.4,
            life: rng.range(1.4, 2.4),
            ch: rng.pick("░▒░"),
            color: hsv(0.0, 0.0, rng.range(0.65, 0.9)),
            leaf: false,
        });
    }

    pub fn flare(&mut self, rng: &mut Rng, x: f32, y: f32) {
        self.0.push(Particle {
            x,
            y,
            vx: rng.range(-2.0, 2.0),
            vy: rng.range(-8.0, -3.0),
            gravity: -1.0,
            life: rng.range(0.3, 0.8),
            ch: rng.pick("'.`^*"),
            color: hsv(rng.range(5.0, 50.0), 0.85, 1.0),
            leaf: false,
        });
    }

    pub fn snore(&mut self, rng: &mut Rng, x: f32, y: f32) {
        self.0.push(Particle {
            x,
            y,
            vx: 1.6,
            vy: -1.1,
            gravity: 0.0,
            life: 2.2,
            ch: if rng.below(2) == 0 { 'z' } else { 'Z' },
            color: DIM,
            leaf: false,
        });
    }

    pub fn carrot(&mut self, rng: &mut Rng, area: Rect) {
        self.0.push(Particle {
            x: rng.range(area.left() as f32, area.right() as f32),
            y: area.top() as f32,
            vx: 0.0,
            vy: rng.range(7.0, 14.0),
            gravity: 0.0,
            life: 6.0,
            ch: '▼',
            color: CARROT,
            leaf: true,
        });
    }
}
