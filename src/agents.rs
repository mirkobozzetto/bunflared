//! Teaches the coding agents installed here to reach for bunflared.

use std::fs;
use std::path::Path;

use crate::os;

const NOTE: &str = "# Sharing a local app online

When the user wants to share, show or demo something running locally (a link
for a client, a phone, a teammate), use bunflared:

    bunflared <port> [more ports] --detach

It prints one JSON line with the public `url` once the link works: give that
to the user. The first port is served at `/`, the others under
`/_port/<port>`. `bunflared ls --json` lists shares, `bunflared down <id>`
stops one. Everything else: `bunflared --help`.
";

// omp loads a rule into every session only when it says so.
const OMP_HEADER: &str = "---
description: Share a local app on a public link with bunflared.
alwaysApply: true
---

";

const START: &str = "<!-- bunflared:start -->";
const END: &str = "<!-- bunflared:end -->";

enum Place {
    /// A file of its own in the harness's rules folder.
    Rule(&'static str, &'static str),
    /// A marked block inside the harness's global AGENTS.md.
    Block(&'static str),
}

struct Harness {
    name: &'static str,
    home: &'static str,
    place: Place,
}

const HARNESSES: [Harness; 5] = [
    Harness {
        name: "Claude Code",
        home: ".claude",
        place: Place::Rule("rules/bunflared.md", ""),
    },
    Harness {
        name: "omp",
        home: ".omp/agent",
        place: Place::Rule("rules/bunflared.md", OMP_HEADER),
    },
    Harness {
        name: "pi",
        home: ".pi/agent",
        place: Place::Block("AGENTS.md"),
    },
    Harness {
        name: "prime-agent",
        home: ".prime/agent",
        place: Place::Block("AGENTS.md"),
    },
    Harness {
        name: "Codex",
        home: ".codex",
        place: Place::Block("AGENTS.md"),
    },
];

pub fn run(print: bool, remove: bool) -> i32 {
    if print {
        print!("{NOTE}");
        return 0;
    }
    let Some(home) = os::home() else {
        eprintln!("No home folder found.");
        return 1;
    };
    let mut touched = 0;
    for harness in &HARNESSES {
        let root = home.join(harness.home);
        if !root.is_dir() {
            continue;
        }
        let (file, done) = match harness.place {
            Place::Rule(relative, header) => {
                let file = root.join(relative);
                let done = if remove {
                    remove_file(&file)
                } else {
                    write_rule(&file, header)
                };
                (file, done)
            }
            Place::Block(relative) => {
                let file = root.join(relative);
                let done = if remove {
                    strip_block(&file)
                } else {
                    write_block(&file)
                };
                (file, done)
            }
        };
        match done {
            Ok(true) => {
                touched += 1;
                let verb = if remove { "removed from" } else { "taught" };
                println!("✓ {:<12} {verb} {}", harness.name, file.display());
            }
            Ok(false) => {}
            Err(err) => println!("✖ {:<12} {}: {err}", harness.name, file.display()),
        }
    }
    if touched == 0 {
        println!(
            "{}",
            if remove {
                "Nothing to remove."
            } else {
                "No known coding agent found here."
            }
        );
    }
    if !remove {
        println!(
            "\nOther agents (Cursor, Grok, ...): paste the output of `bunflared agents --print` into their rules."
        );
    }
    0
}

fn write_rule(file: &Path, header: &str) -> std::io::Result<bool> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(file, format!("{header}{NOTE}"))?;
    Ok(true)
}

fn remove_file(file: &Path) -> std::io::Result<bool> {
    if !file.exists() {
        return Ok(false);
    }
    fs::remove_file(file)?;
    Ok(true)
}

/// Replaces the marked block, or appends it. Writing through the path keeps
/// an AGENTS.md that is a symlink into dotfiles a symlink.
fn write_block(file: &Path) -> std::io::Result<bool> {
    let current = fs::read_to_string(file).unwrap_or_default();
    let block = format!("{START}\n{NOTE}{END}\n");
    let updated = match span(&current) {
        Some((start, end)) => format!("{}{block}{}", &current[..start], &current[end..]),
        None if current.trim().is_empty() => block,
        None => format!("{}\n\n{block}", current.trim_end()),
    };
    fs::write(file, updated)?;
    Ok(true)
}

fn strip_block(file: &Path) -> std::io::Result<bool> {
    let Ok(current) = fs::read_to_string(file) else {
        return Ok(false);
    };
    let Some((start, end)) = span(&current) else {
        return Ok(false);
    };
    let rest = format!("{}\n{}", current[..start].trim_end(), &current[end..]);
    fs::write(file, rest.trim_start_matches('\n'))?;
    Ok(true)
}

/// Byte range of the block, including the newline that ends it.
fn span(text: &str) -> Option<(usize, usize)> {
    let start = text.find(START)?;
    let end = start + text[start..].find(END)? + END.len();
    let end = if text[end..].starts_with('\n') {
        end + 1
    } else {
        end
    };
    Some((start, end))
}
