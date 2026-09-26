//! Effects laid over the drawn frame with tachyonfx, each on a named spot of
//! the dashboard. Nothing runs, and nothing costs, while the list is empty.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tachyonfx::fx::{self, Glitch};
use tachyonfx::{Duration, Effect, Interpolation, IntoEffect, Motion};

use super::fx::PINK;

pub const DISSOLVE_MS: u32 = 700;

#[derive(Clone, Copy)]
enum Spot {
    Screen,
    Visitors,
    Log,
    Qr,
}

/// Where each spot sits in the frame being drawn; `None` when not on screen.
#[derive(Default)]
pub struct Spots {
    pub visitors: Option<Rect>,
    pub log: Option<Rect>,
    pub qr: Option<Rect>,
}

pub struct Shaders {
    calm: bool,
    running: Vec<(Spot, Effect)>,
    disco: Option<Effect>,
}

impl Shaders {
    pub fn new(calm: bool) -> Self {
        Self {
            calm,
            running: Vec::new(),
            disco: None,
        }
    }

    fn add(&mut self, spot: Spot, effect: Effect) {
        if !self.calm {
            self.running.push((spot, effect));
        }
    }

    /// The panels materialize when the dashboard opens.
    pub fn entry(&mut self) {
        self.add(Spot::Screen, fx::coalesce((700, Interpolation::QuadOut)));
    }

    /// A pink sweep over the visitors when someone arrives.
    pub fn arrival(&mut self) {
        let sweep = fx::sweep_in(
            Motion::LeftToRight,
            12,
            0,
            PINK,
            (900, Interpolation::QuadOut),
        );
        self.add(Spot::Visitors, sweep);
    }

    /// The request log glitches for a moment on a 5xx.
    pub fn crash(&mut self) {
        let glitch = Glitch::builder()
            .cell_glitch_ratio(0.08)
            .action_start_delay_ms(0..120)
            .action_ms(60..200)
            .build()
            .into_effect();
        let brief = fx::with_duration(Duration::from_millis(600), glitch);
        self.add(Spot::Log, brief);
    }

    /// The QR panel flashes when `r` asks for a code already on screen.
    pub fn flash_qr(&mut self) {
        self.add(
            Spot::Qr,
            fx::fade_from(PINK, PINK, (600, Interpolation::QuadOut)),
        );
    }

    /// The dashboard dissolves before the goodbye.
    pub fn dissolve(&mut self) {
        self.add(
            Spot::Screen,
            fx::dissolve((DISSOLVE_MS, Interpolation::Linear)),
        );
    }

    /// Disco mode: the hue of the whole screen keeps turning.
    pub fn disco(&mut self, on: bool) {
        self.disco = (on && !self.calm).then(|| {
            fx::repeating(fx::hsl_shift_fg(
                [360.0, 20.0, 0.0],
                (2400, Interpolation::Linear),
            ))
        });
    }

    pub fn busy(&self) -> bool {
        !self.running.is_empty() || self.disco.is_some()
    }

    pub fn render(&mut self, buf: &mut Buffer, screen: Rect, spots: &Spots, dt: f32) {
        let tick = Duration::from_millis((dt * 1000.0) as u32);
        self.running.retain_mut(|(spot, effect)| {
            let area = match spot {
                Spot::Screen => Some(screen),
                Spot::Visitors => spots.visitors,
                Spot::Log => spots.log,
                Spot::Qr => spots.qr,
            };
            // A spot off screen drops its effect: it would play late or nowhere.
            let Some(area) = area else {
                return false;
            };
            effect.process(tick, buf, area);
            !effect.done()
        });
        if let Some(disco) = &mut self.disco {
            disco.process(tick, buf, screen);
        }
    }
}
