use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

use super::{big_text, centered, hint};
use crate::tui::App;
use crate::tui::art;
use crate::tui::cast;
use crate::tui::fx::{self, center, put};

const TAGLINE: &str = "share localhost · dig a tunnel · feed the bunny";

// Two laps across the screen during the boot.
const DASH_LAP: f32 = 0.75;
const FIRE_ROWS: usize = 12;

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
