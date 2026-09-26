pub type Frame3 = [&'static str; 3];

pub const IDLE: Frame3 = [r" (\(\ ", r" ( •.•)", r#" o_(")(")"#];
pub const BLINK: Frame3 = [r" (\(\ ", r" ( -.-)", r#" o_(")(")"#];
pub const HAPPY: Frame3 = [r" (\(\ ", r" ( ^.^)", r#" o_(")(")"#];
pub const HOP: Frame3 = [r" (\(\ ", r" ( ^o^)", r#"  (")(")"#];
pub const SLEEP: Frame3 = [r" (\(\ ", r" ( -.-)", r#" o_(")(")"#];
pub const PANIC_A: Frame3 = [r"  \\//", r" ( O_O)!!", r#" o_(")(")"#];
pub const PANIC_B: Frame3 = [r"  //\\", r" ( O_O)!!", r#" o_(")(")"#];
pub const WORRIED_A: Frame3 = [r" (\(\ ", r" ( •~•)", r#" o_(")(")"#];
pub const WORRIED_B: Frame3 = [r" (\(\ '", r" ( •~•)", r#" o_(")(")"#];
pub const SAD: Frame3 = [r" (\(\ ", r" ( ;_;)", r#" o_(")(")"#];
pub const WAVE_A: Frame3 = [r" (\(\  ", r" ( ^.^)/", r#" o_(")(")"#];
pub const WAVE_B: Frame3 = [r" (\(\  ", r" ( ^.^)_", r#" o_(")(")"#];
pub const DANCE_A: Frame3 = [r"  (\(\", r#"\( ^o^)"#, r#"  (")(")/"#];
pub const DANCE_B: Frame3 = [r" /)/)", r" (^o^ )/", r#"\(")(")"#];
pub const FIREFIGHTER_THROW: Frame3 = [r" (\(\ ", r" (>.<)", r#" o_(")(")"#];
pub const PHOENIX_UP: [&str; 5] = [
    r"\\\\         ////",
    r" \\\\   ^   //// ",
    r"   \\\ (o) ///   ",
    r"      ~/V\~      ",
    r"       ' '       ",
];
pub const PHOENIX_DOWN: [&str; 5] = [
    r"        ^        ",
    r"  ____ (o) ____  ",
    r" ///\\ /V\ //\\\ ",
    r"///    ' '    \\\",
    r"                 ",
];
pub const RUN_A: Frame3 = [r" /)/)", r" (•.• )", r#"(")(")~"#];
pub const RUN_B: Frame3 = [r" /)/)", r" (•.• )", r#" (")(") ~"#];

const GLYPHS: [(char, [&str; 5]); 12] = [
    ('B', ["████ ", "█   █", "████ ", "█   █", "████ "]),
    ('U', ["█   █", "█   █", "█   █", "█   █", " ███ "]),
    ('N', ["█   █", "██  █", "█ █ █", "█  ██", "█   █"]),
    ('F', ["█████", "█    ", "████ ", "█    ", "█    "]),
    ('L', ["█    ", "█    ", "█    ", "█    ", "█████"]),
    ('A', [" ███ ", "█   █", "█████", "█   █", "█   █"]),
    ('R', ["████ ", "█   █", "████ ", "█  █ ", "█   █"]),
    ('E', ["█████", "█    ", "████ ", "█    ", "█████"]),
    ('D', ["████ ", "█   █", "█   █", "█   █", "████ "]),
    ('I', ["███", " █ ", " █ ", " █ ", "███"]),
    ('V', ["█   █", "█   █", "█   █", " █ █ ", "  █  "]),
    ('!', ["█", "█", "█", " ", "█"]),
];

/// Five rows of block letters, one space between glyphs.
pub fn big(text: &str) -> Vec<String> {
    let mut rows = vec![String::new(); 5];
    for (i, ch) in text.chars().enumerate() {
        let Some((_, glyph)) = GLYPHS.iter().find(|(c, _)| *c == ch) else {
            continue;
        };
        for (row, line) in rows.iter_mut().zip(glyph) {
            if i > 0 {
                row.push(' ');
            }
            row.push_str(line);
        }
    }
    rows
}

pub const JOKES: [&str; 12] = [
    "Negotiating with Cloudflare's edge...",
    "Bribing the DNS gnomes with carrots...",
    "Asking 1.1.1.1 very nicely...",
    "Teaching packets to hop...",
    "Untangling the internet's cables...",
    "Polishing the TLS certificate...",
    "Warming up the tubes...",
    "Looking for the DNS record under the couch...",
    "Convincing the router this is fine...",
    "Digging past a very confused mole...",
    "Reticulating splines, the rabbit kind...",
    "Hopping over a firewall...",
];

pub struct Achievement {
    pub key: &'static str,
    pub name: &'static str,
    pub blurb: &'static str,
}

pub const ACHIEVEMENTS: [Achievement; 14] = [
    Achievement {
        key: "first",
        name: "First visitor!",
        blurb: "Someone opened your link.",
    },
    Achievement {
        key: "party",
        name: "Party of five",
        blurb: "Five different visitors.",
    },
    Achievement {
        key: "centurion",
        name: "Centurion",
        blurb: "100 requests served.",
    },
    Achievement {
        key: "thousand",
        name: "Carrot tycoon",
        blurb: "1000 requests served.",
    },
    Achievement {
        key: "speed",
        name: "Speed demon",
        blurb: "Answered in under 10 ms.",
    },
    Achievement {
        key: "survivor",
        name: "Survivor",
        blurb: "Took a 5xx and kept hopping.",
    },
    Achievement {
        key: "tube",
        name: "Tube wrangler",
        blurb: "A WebSocket went through.",
    },
    Achievement {
        key: "marathon",
        name: "Marathon bunny",
        blurb: "One hour live.",
    },
    Achievement {
        key: "owl",
        name: "Night owl",
        blurb: "Sharing after midnight.",
    },
    Achievement {
        key: "disco",
        name: "Disco inferno",
        blurb: "You know the code.",
    },
    Achievement {
        key: "carrot",
        name: "Carrot cake",
        blurb: "It's raining carrots.",
    },
    Achievement {
        key: "stampede",
        name: "Stampede",
        blurb: "Release the bunnies.",
    },
    Achievement {
        key: "pyro",
        name: "Pyromaniac",
        blurb: "Ten fireworks. Easy there.",
    },
    Achievement {
        key: "critic",
        name: "Word from the client",
        blurb: "Someone left feedback.",
    },
];

pub fn achievement(key: &str) -> &'static Achievement {
    ACHIEVEMENTS
        .iter()
        .find(|a| a.key == key)
        .expect("known achievement")
}
