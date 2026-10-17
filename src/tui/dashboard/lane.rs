use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::text::{Line, Span};

use super::panel;
use crate::live::REACTIONS;
use crate::tui::App;
use crate::tui::art::{self, Frame3};
use crate::tui::fx::{self, put};
use crate::tui::show::LANE_TRIP;

const PANIC_FOR: f32 = 3.0;
const HOP_FOR: f32 = 1.2;

struct Mood {
    label: &'static str,
    pose: &'static Frame3,
    jitter: i32,
    color: Color,
}

fn mood(app: &App) -> Mood {
    let e = app.elapsed();
    let alt = |hz: f32| !app.theme.calm && (e * hz) as i32 % 2 == 1;
    let recent = |at: Option<std::time::Instant>, window: f32| {
        at.is_some_and(|at| (app.now - at).as_secs_f32() < window)
    };
    if app.show.disco {
        let pose = if alt(4.0) {
            &art::DANCE_B
        } else {
            &art::DANCE_A
        };
        Mood {
            label: "disco!",
            pose,
            jitter: 0,
            color: fx::rainbow(e * 200.0),
        }
    } else if app.show.worried_until.is_some() {
        let pose = if alt(3.0) {
            &art::WORRIED_B
        } else {
            &art::WORRIED_A
        };
        Mood {
            label: "worried",
            pose,
            jitter: 0,
            color: fx::YELLOW,
        }
    } else if recent(app.traffic.last_error, PANIC_FOR) {
        let pose = if alt(7.0) {
            &art::PANIC_B
        } else {
            &art::PANIC_A
        };
        Mood {
            label: "PANICKING",
            pose,
            jitter: if alt(11.0) { 1 } else { 0 },
            color: fx::RED,
        }
    } else if recent(app.traffic.last_hit, HOP_FOR) {
        let pose = if alt(5.0) { &art::HOP } else { &art::HAPPY };
        Mood {
            label: "hopping",
            pose,
            jitter: 0,
            color: fx::FG,
        }
    } else if app.sleeping() {
        Mood {
            label: "napping",
            pose: &art::SLEEP,
            jitter: 0,
            color: fx::DIM,
        }
    } else {
        let blink = !app.theme.calm && e % 3.2 < 0.15;
        Mood {
            label: "chilling",
            pose: if blink { &art::BLINK } else { &art::IDLE },
            jitter: 0,
            color: fx::FG,
        }
    }
}

pub fn draw_lane(app: &mut App, frame: &mut Frame, area: Rect) {
    let mood = mood(app);
    let caption = Line::from(Span::styled(
        format!(" bunny: {} ", mood.label),
        app.theme.fg(mood.color),
    ))
    .right_aligned();
    let mut block = panel(app, "traffic").title_bottom(caption);
    if app.audience.reactions.iter().any(|&n| n > 0) {
        let counts: Vec<String> = REACTIONS
            .iter()
            .zip(app.audience.reactions)
            .map(|((emoji, _), n)| format!("{emoji} {n}"))
            .collect();
        block = block.title_bottom(Span::styled(
            format!(" {} ", counts.join("  ")),
            app.theme.fg(fx::FG),
        ));
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let e = app.elapsed();
    let buf = frame.buffer_mut();
    let theme = &app.theme;
    let (left, right) = (inner.x as i32 + 12, inner.right() as i32 - 2);
    put(
        buf,
        inner.x as i32 + 1,
        inner.y as i32,
        "⌂ your machine",
        theme.fg(fx::DIM),
    );
    put(
        buf,
        right - 13,
        inner.y as i32,
        "the internet ☁",
        theme.fg(fx::DIM),
    );

    let offset = if theme.calm { 0 } else { (e * 10.0) as i32 };
    for lane in 0..3 {
        let y = inner.y as i32 + 1 + lane;
        for x in left..right {
            if (x - offset).rem_euclid(4) == 0 {
                put(
                    buf,
                    x,
                    y,
                    "·",
                    theme.fg(fx::DIM).add_modifier(Modifier::DIM),
                );
            }
        }
    }
    for sprite in &app.show.sprites {
        let trip = (app.now - sprite.born).as_secs_f32() / LANE_TRIP;
        let x = left + ((right - left) as f32 * trip) as i32;
        let y = inner.y as i32 + 1 + sprite.lane as i32;
        let color = if app.show.disco {
            fx::rainbow(x as f32 * 8.0)
        } else {
            sprite.color
        };
        for (back, ch) in [(2, "·"), (1, "•")] {
            if x - back >= left {
                put(
                    buf,
                    x - back,
                    y,
                    ch,
                    theme.fg(color).add_modifier(Modifier::DIM),
                );
            }
        }
        put(
            buf,
            x,
            y,
            &sprite.glyph.to_string(),
            theme.fg(color).add_modifier(Modifier::BOLD),
        );
    }
    fx::art(
        buf,
        inner.x as i32 + mood.jitter,
        inner.y as i32 + 1,
        mood.pose,
        theme.fg(mood.color),
    );

    if !theme.calm && mood.label == "napping" && app.show.rng.range(0.0, 1.0) < app.dt * 1.2 {
        app.show.particles.snore(
            &mut app.show.rng,
            inner.x as f32 + 9.0,
            inner.y as f32 + 1.0,
        );
    }
}
