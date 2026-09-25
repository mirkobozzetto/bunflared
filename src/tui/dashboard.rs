use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use super::art::{self, Frame3};
use super::fx::{self, put};
use super::{App, LANE_TRIP, Qr, Theme, plural};

const MIN_WIDTH: u16 = 64;
const MIN_HEIGHT: u16 = 20;
const QR_MIN_MAIN: u16 = 56;
const ECG: [char; 16] = [
    '▁', '▁', '▁', '▂', '▁', '▁', '▇', '▃', '▁', '▁', '▁', '▁', '▁', '▁', '▁', '▁',
];
const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
const PANIC_FOR: f32 = 3.0;
const HOP_FOR: f32 = 1.2;
const KEYS: [(&str, &str); 6] = [
    ("c", "copy"),
    ("o", "open"),
    ("r", "qr"),
    ("f", "fireworks"),
    ("?", "help"),
    ("q", "quit"),
];
const HELP: [&str; 9] = [
    "c   copy the link",
    "o   open it in your browser",
    "r   big QR code for phones",
    "f   fireworks",
    "?   this help",
    "q   stop sharing",
    "",
    "psst: there are secrets.",
    "the bunny knows a few codes.",
];

fn hms(secs: u64) -> String {
    format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

fn panel<'a>(app: &App, title: &str) -> Block<'a> {
    let border = if app.disco {
        fx::rainbow(app.elapsed() * 150.0 + title.len() as f32 * 45.0)
    } else {
        fx::DIM
    };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(app.theme.fg(border))
        .title(Span::styled(
            format!(" {title} "),
            app.theme.fg(fx::PINK).add_modifier(Modifier::BOLD),
        ))
}

pub fn render(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        return compact(app, frame, area);
    }
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    draw_header(app, frame, header);

    let qr_size = app.qr_code.as_ref().map_or(0, |qr| qr.size as u16);
    let (qr_width, qr_height) = (qr_size + 2, qr_size.div_ceil(2) + 2);
    let fits = qr_size > 0 && body.width >= qr_width + QR_MIN_MAIN && body.height >= qr_height;
    let main = if fits {
        let [main, side] =
            Layout::horizontal([Constraint::Min(0), Constraint::Length(qr_width)]).areas(body);
        let [side, _] =
            Layout::vertical([Constraint::Length(qr_height), Constraint::Min(0)]).areas(side);
        let block = panel(app, "scan me");
        let inner = block.inner(side);
        frame.render_widget(block, side);
        if let Some(qr) = &app.qr_code {
            draw_qr(
                frame.buffer_mut(),
                &app.theme,
                qr,
                inner.x as i32,
                inner.y as i32,
            );
        }
        app.keep_clear = Some(inner);
        main
    } else {
        body
    };

    let [lane, middle, log] = Layout::vertical([
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Min(3),
    ])
    .areas(main);
    let [ports, stats] =
        Layout::horizontal([Constraint::Percentage(48), Constraint::Percentage(52)]).areas(middle);
    draw_lane(app, frame, lane);
    draw_ports(app, frame, ports);
    draw_stats(app, frame, stats);
    draw_log(app, frame, log);
    draw_footer(app, frame, footer);
}

fn draw_header(app: &App, frame: &mut Frame, area: Rect) {
    let e = app.elapsed();
    let title: Vec<Span> = " bunflared "
        .chars()
        .enumerate()
        .map(|(i, ch)| {
            let color = fx::rainbow(i as f32 * 30.0 + e * 90.0);
            Span::styled(
                ch.to_string(),
                app.theme.fg(color).add_modifier(Modifier::BOLD),
            )
        })
        .collect();
    let dot = if app.theme.calm || (e * 2.0) as i32 % 2 == 0 {
        fx::RED
    } else {
        fx::PINK
    };
    let live = Line::from(vec![
        Span::styled(" ●", app.theme.fg(dot)),
        Span::styled(
            format!(" LIVE {} ", hms(app.live_for())),
            app.theme.fg(fx::FG).add_modifier(Modifier::BOLD),
        ),
    ])
    .right_aligned();
    let border = if app.disco {
        fx::rainbow(e * 150.0)
    } else {
        fx::DIM
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(app.theme.fg(border))
        .title(Line::from(title))
        .title_top(live);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let url = app.url.clone().unwrap_or_default();
    let (note, color) = if app.copied {
        ("✓ in your clipboard", fx::GREEN)
    } else {
        ("press c to copy", fx::DIM)
    };
    let line = Line::from(vec![
        Span::styled(
            url,
            app.theme
                .fg(fx::CYAN)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ),
        Span::raw("   "),
        Span::styled(note, app.theme.fg(color)),
    ]);
    frame.render_widget(Paragraph::new(line), inner);
}

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
    if app.disco {
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
    } else if recent(app.last_error, PANIC_FOR) {
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
    } else if recent(app.last_hit, HOP_FOR) {
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

fn draw_lane(app: &mut App, frame: &mut Frame, area: Rect) {
    let mood = mood(app);
    let caption = Line::from(Span::styled(
        format!(" bunny: {} ", mood.label),
        app.theme.fg(mood.color),
    ))
    .right_aligned();
    let block = panel(app, "traffic").title_bottom(caption);
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
    for sprite in &app.sprites {
        let trip = (app.now - sprite.born).as_secs_f32() / LANE_TRIP;
        let x = left + ((right - left) as f32 * trip) as i32;
        let y = inner.y as i32 + 1 + sprite.lane as i32;
        let color = if app.disco {
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

    if !theme.calm && mood.label == "napping" && app.rng.range(0.0, 1.0) < app.dt * 1.2 {
        app.particles
            .snore(&mut app.rng, inner.x as f32 + 9.0, inner.y as f32 + 1.0);
    }
}

fn draw_ports(app: &App, frame: &mut Frame, area: Rect) {
    let block = panel(app, "ports");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let e = app.elapsed();
    let shift = if app.theme.calm {
        0
    } else {
        (e * 8.0) as usize
    };
    let lines: Vec<Line> = app
        .ports
        .iter()
        .enumerate()
        .map(|(i, port)| {
            let target = format!(" {:<12} :{:<5} ", port.route, port.port);
            if port.up {
                let beat = !app.theme.calm && (e * 1.3 + i as f32 * 0.37).fract() < 0.25;
                let heart = if beat {
                    app.theme.fg(fx::RED).add_modifier(Modifier::BOLD)
                } else {
                    app.theme.fg(fx::PINK)
                };
                let ecg: String = (0..10)
                    .map(|j| ECG[(j + shift + i * 5) % ECG.len()])
                    .collect();
                Line::from(vec![
                    Span::styled("♥", heart),
                    Span::styled(target, app.theme.fg(fx::CYAN)),
                    Span::styled(ecg, app.theme.fg(fx::GREEN)),
                    Span::styled(format!(" {}", port.hits), app.theme.fg(fx::DIM)),
                ])
            } else {
                Line::from(vec![
                    Span::styled("✖", app.theme.fg(fx::RED).add_modifier(Modifier::BOLD)),
                    Span::styled(target, app.theme.fg(fx::CYAN)),
                    Span::styled("▁".repeat(10), app.theme.fg(fx::RED)),
                    Span::styled(" down", app.theme.fg(fx::RED).add_modifier(Modifier::BOLD)),
                ])
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_stats(app: &App, frame: &mut Frame, area: Rect) {
    let block = panel(app, "stats");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let theme = &app.theme;
    let average = if app.total == 0 {
        0
    } else {
        app.ms_sum / app.total as u64
    };
    let strong = |value: String, color: Color| {
        Span::styled(value, theme.fg(color).add_modifier(Modifier::BOLD))
    };
    let label = |text: &'static str| Span::styled(text, theme.fg(fx::DIM));
    let mut lines = vec![
        Line::from(vec![
            strong(app.total.to_string(), fx::FG),
            label(" requests  "),
            strong(app.visitors.len().to_string(), fx::FG),
            label(if app.visitors.len() == 1 {
                " visitor  "
            } else {
                " visitors  "
            }),
            strong(app.sockets.to_string(), fx::FG),
            label(" sockets"),
        ]),
        Line::from(vec![
            label("2xx "),
            strong(app.classes[0].to_string(), fx::GREEN),
            label("  3xx "),
            strong(app.classes[1].to_string(), fx::CYAN),
            label("  4xx "),
            strong(app.classes[2].to_string(), fx::YELLOW),
            label("  5xx "),
            strong(app.classes[3].to_string(), fx::RED),
        ]),
        Line::from(vec![
            label("avg "),
            strong(format!("{average} ms"), fx::FG),
            label("  carrots eaten "),
            strong(app.classes[0].to_string(), fx::CARROT),
        ]),
    ];
    let width = inner.width as usize;
    let recent: Vec<u64> = app
        .per_second
        .iter()
        .rev()
        .take(width)
        .rev()
        .copied()
        .collect();
    let peak = recent.iter().copied().max().unwrap_or(0).max(1);
    let spark: Vec<Span> = recent
        .iter()
        .map(|&count| {
            let level = if count == 0 {
                0
            } else {
                1 + (count * 6 / peak) as usize
            };
            Span::styled(
                BARS[level.min(7)].to_string(),
                theme.fg(fx::hsv(140.0 - level as f32 * 18.0, 0.7, 1.0)),
            )
        })
        .collect();
    lines.push(Line::from(spark));
    frame.render_widget(Paragraph::new(lines), inner);
}

fn status_color(status: u16) -> Color {
    match status {
        0..300 => fx::GREEN,
        300..400 => fx::CYAN,
        400..500 => fx::YELLOW,
        _ => fx::RED,
    }
}

fn draw_log(app: &App, frame: &mut Frame, area: Rect) {
    let block = panel(app, "requests");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let theme = &app.theme;
    if app.rows.is_empty() {
        let waiting = Line::from(Span::styled(
            "No visitors yet. Send the link to someone!",
            theme.fg(fx::DIM),
        ))
        .centered();
        let [_, middle, _] = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(inner);
        frame.render_widget(Paragraph::new(waiting), middle);
        return;
    }
    let lines: Vec<Line> = app
        .rows
        .iter()
        .take(inner.height as usize)
        .map(|row| {
            let hit = &row.hit;
            Line::from(vec![
                Span::styled(format!("{} ", row.clock), theme.fg(fx::DIM)),
                Span::styled(format!("{:<7}", hit.method), theme.fg(fx::PURPLE)),
                Span::styled(
                    format!("{} ", hit.status),
                    theme
                        .fg(status_color(hit.status))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{:>5}ms ", hit.ms), theme.fg(fx::FG)),
                Span::styled(format!(":{:<5} ", hit.port), theme.fg(fx::DIM)),
                Span::styled(hit.path.clone(), theme.fg(fx::FG)),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_footer(app: &App, frame: &mut Frame, area: Rect) {
    let mut spans = vec![Span::raw(" ")];
    for (key, label) in KEYS {
        spans.push(Span::styled(
            key,
            app.theme.fg(fx::PINK).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(format!(" {label}   "), app.theme.fg(fx::DIM)));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn compact(app: &App, frame: &mut Frame, area: Rect) {
    let theme = &app.theme;
    let status = if app.ready_at.is_some() {
        format!("● LIVE {}", hms(app.live_for()))
    } else {
        "digging...".into()
    };
    let lines = vec![
        Line::from(vec![
            Span::styled(
                "bunflared ",
                theme.fg(fx::PINK).add_modifier(Modifier::BOLD),
            ),
            Span::styled(status, theme.fg(fx::RED)),
        ]),
        Line::from(Span::styled(
            app.url.clone().unwrap_or_default(),
            theme.fg(fx::CYAN).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!(
                "{} · {} · {}",
                plural(app.total as usize, "request"),
                plural(app.visitors.len(), "visitor"),
                plural(app.classes[3] as usize, "error")
            ),
            theme.fg(fx::FG),
        )),
        Line::from(Span::styled(
            "c copy · o open · r qr · q quit",
            theme.fg(fx::DIM),
        )),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}

/// Half blocks with explicit black and white, so phones scan it on any theme.
fn draw_qr(buf: &mut Buffer, theme: &Theme, qr: &Qr, x: i32, y: i32) {
    const BLACK: Color = Color::Rgb(0, 0, 0);
    const WHITE: Color = Color::Rgb(255, 255, 255);
    let dark = |col: usize, row: usize| row < qr.size && qr.dark[row * qr.size + col];
    for row in 0..qr.size.div_ceil(2) {
        for col in 0..qr.size {
            let (top, bottom) = (dark(col, row * 2), dark(col, row * 2 + 1));
            let (cx, cy) = (x + col as i32, y + row as i32);
            if theme.color {
                let fg = if top { BLACK } else { WHITE };
                let bg = if bottom { BLACK } else { WHITE };
                put(buf, cx, cy, "▀", Style::new().fg(fg).bg(bg));
            } else {
                // ponytail: assumes a dark terminal, light modules are drawn.
                let ch = match (top, bottom) {
                    (false, false) => "█",
                    (false, true) => "▀",
                    (true, false) => "▄",
                    (true, true) => " ",
                };
                put(buf, cx, cy, ch, Style::new());
            }
        }
    }
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

pub fn overlays(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    if app.qr
        && let Some(qr) = &app.qr_code
    {
        let rect = centered_rect(area, qr.size as u16 + 4, qr.size.div_ceil(2) as u16 + 4);
        frame.render_widget(Clear, rect);
        let block = panel(app, "scan me").title_bottom(Line::from(" esc to close ").centered());
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        draw_qr(
            frame.buffer_mut(),
            &app.theme,
            qr,
            inner.x as i32 + 1,
            inner.y as i32 + 1,
        );
        app.keep_clear = Some(rect);
    }
    if app.help {
        let rect = centered_rect(area, 38, HELP.len() as u16 + 4);
        frame.render_widget(Clear, rect);
        let block = panel(app, "keys");
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        let lines: Vec<Line> = HELP
            .iter()
            .map(|line| {
                let color = if line.starts_with("psst") || line.starts_with("the bunny") {
                    fx::YELLOW
                } else {
                    fx::FG
                };
                Line::from(Span::styled(format!(" {line}"), app.theme.fg(color)))
            })
            .collect();
        frame.render_widget(
            Paragraph::new(lines),
            inner.inner(ratatui::layout::Margin::new(0, 1)),
        );
    }
    for (i, toast) in app.toasts.iter().enumerate() {
        let width = (toast.title.chars().count().max(toast.body.chars().count()) as u16 + 4)
            .min(area.width);
        let age = (app.now - toast.born).as_secs_f32();
        let slide = if app.theme.calm {
            0
        } else {
            ((1.0 - (age / 0.25).min(1.0)) * width as f32) as u16
        };
        let x = area.right().saturating_sub(width + 1) + slide;
        // Stacked up from the footer, clear of the QR code at the top right.
        let Some(y) = area.bottom().checked_sub(2 + (i as u16 + 1) * 4) else {
            continue;
        };
        if y < area.y || x >= area.right() {
            continue;
        }
        let rect = Rect::new(x, y, width.min(area.right() - x), 4);
        frame.render_widget(Clear, rect);
        let border = if toast.gold { fx::GOLD } else { fx::CYAN };
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(app.theme.fg(border));
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        let lines = vec![
            Line::from(Span::styled(
                toast.title.clone(),
                app.theme.fg(border).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(toast.body.clone(), app.theme.fg(fx::FG))),
        ];
        frame.render_widget(Paragraph::new(lines), inner);
    }
}
