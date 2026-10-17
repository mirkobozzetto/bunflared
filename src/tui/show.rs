use std::time::{Duration, Instant};

use ratatui::layout::Rect;
use ratatui::style::Color;

use super::traffic::class;
use super::{art, fx, shaders, spectacle};
use crate::event::Hit;

pub const LANE_TRIP: f32 = 1.4;
const TOAST_LIFE: f32 = 3.5;
const PYRO: u32 = 10;
const WORRY_FOR: Duration = Duration::from_secs(3);
// Past this, a reaction still counts and toasts, without more confetti.
const MAX_PARTICLES: usize = 800;

pub struct Sprite {
    pub born: Instant,
    pub lane: u16,
    pub glyph: char,
    pub color: Color,
}

pub struct Toast {
    pub title: String,
    pub body: String,
    pub born: Instant,
    pub gold: bool,
}

pub struct Runner {
    pub x: f32,
    pub y: f32,
    pub speed: f32,
}

/// Everything on screen for fun: particles, sprites, toasts, shaders,
/// the achievements and what the secret codes start.
pub struct Show {
    calm: bool,
    pub rng: fx::Rng,
    pub particles: fx::Particles,
    pub fire: fx::Fire,
    pub spectacle: spectacle::Spectacle,
    pub shaders: shaders::Shaders,
    pub spots: shaders::Spots,
    pub sprites: Vec<Sprite>,
    pub toasts: Vec<Toast>,
    pub runners: Vec<Runner>,
    pub launch_bursts: u8,
    pub disco: bool,
    fireworks: u32,
    pub carrots_until: Option<Instant>,
    pub worried_until: Option<Instant>,
    pub unlocked: Vec<&'static art::Achievement>,
    /// Particles skip this area: the QR code, the freshly announced link.
    pub keep_clear: Option<Rect>,
}

fn area() -> Rect {
    ratatui::crossterm::terminal::size()
        .map(|(w, h)| Rect::new(0, 0, w, h))
        .unwrap_or(Rect::new(0, 0, 80, 24))
}

impl Show {
    pub fn new(calm: bool) -> Self {
        Self {
            calm,
            rng: fx::Rng::seeded(),
            particles: fx::Particles::default(),
            fire: fx::Fire::default(),
            spectacle: spectacle::Spectacle::default(),
            shaders: shaders::Shaders::new(calm),
            spots: shaders::Spots::default(),
            sprites: Vec::new(),
            toasts: Vec::new(),
            runners: Vec::new(),
            launch_bursts: 0,
            disco: false,
            fireworks: 0,
            carrots_until: None,
            worried_until: None,
            unlocked: Vec::new(),
            keep_clear: None,
        }
    }

    /// Something still moves: animations need 30 fps.
    pub fn busy(&self) -> bool {
        !self.particles.is_empty()
            || !self.sprites.is_empty()
            || !self.toasts.is_empty()
            || !self.runners.is_empty()
            || self.disco
            || self.carrots_until.is_some()
            || self.shaders.busy()
    }

    pub fn tick(&mut self, now: Instant, dt: f32) {
        self.particles.step(dt);
        self.sprites
            .retain(|s| (now - s.born).as_secs_f32() < LANE_TRIP);
        self.toasts
            .retain(|toast| (now - toast.born).as_secs_f32() < TOAST_LIFE);
        for runner in &mut self.runners {
            runner.x -= runner.speed * dt;
        }
        self.runners.retain(|runner| runner.x > -12.0);
        if self.carrots_until.is_some_and(|until| now >= until) {
            self.carrots_until = None;
        }
        if self.worried_until.is_some_and(|until| now >= until) {
            self.worried_until = None;
        }
    }

    pub fn toast(&mut self, now: Instant, title: String, body: String, gold: bool) {
        self.toasts.push(Toast {
            title,
            body,
            born: now,
            gold,
        });
    }

    pub fn unlock(&mut self, now: Instant, key: &str) {
        let achievement = art::achievement(key);
        if self.unlocked.iter().any(|a| a.key == key) {
            return;
        }
        self.unlocked.push(achievement);
        let title = format!("★ {}", achievement.name);
        self.toast(now, title, achievement.blurb.into(), true);
        if !self.calm {
            let area = area();
            let (x, y) = (area.width as f32 / 3.0, area.height as f32 - 4.0);
            self.particles.fireworks(&mut self.rng, x, y);
        }
    }

    /// A request runs down the traffic lane.
    pub fn sprite(&mut self, now: Instant, hit: &Hit) {
        if self.calm || self.sprites.len() >= 80 {
            return;
        }
        let (glyph, color) = match (hit.upgrade, class(hit.status)) {
            (true, 0) => ('≈', fx::PURPLE),
            (_, 0) => ('●', fx::GREEN),
            (_, 1) => ('◆', fx::CYAN),
            (_, 2) => ('▲', fx::YELLOW),
            _ => ('✖', fx::RED),
        };
        let lane = self.rng.below(3) as u16;
        self.sprites.push(Sprite {
            born: now,
            lane,
            glyph,
            color,
        });
    }

    /// Each reaction gets its own show.
    pub fn celebrate(&mut self, now: Instant, kind: usize) {
        if self.calm || self.particles.0.len() >= MAX_PARTICLES {
            return;
        }
        let area = area();
        let x = self
            .rng
            .range(10.0, area.width.saturating_sub(10).max(11) as f32);
        let y = area.height as f32 * 0.4;
        match kind {
            0 => self.particles.confetti(&mut self.rng, x, y, 60),
            1 => self.particles.fireworks(&mut self.rng, x, y),
            2 => {
                let bottom = area.height.saturating_sub(4) as f32;
                self.particles.hearts(&mut self.rng, x, bottom);
            }
            _ => self.worried_until = Some(now + WORRY_FOR),
        }
    }

    /// One firework somewhere; true at the tenth.
    pub fn firework(&mut self) -> bool {
        let area = area();
        let x = self.rng.range(8.0, area.width.saturating_sub(8) as f32);
        let y = self
            .rng
            .range(3.0, area.height.saturating_sub(4).max(4) as f32);
        self.particles.fireworks(&mut self.rng, x, y);
        self.fireworks += 1;
        self.fireworks == PYRO
    }

    pub fn stampede(&mut self) {
        let area = area();
        for _ in 0..10 {
            let x = area.width as f32 + self.rng.range(0.0, 40.0);
            let y = self
                .rng
                .range(1.0, area.height.saturating_sub(4).max(2) as f32);
            let speed = self.rng.range(25.0, 55.0);
            self.runners.push(Runner { x, y, speed });
        }
    }
}
