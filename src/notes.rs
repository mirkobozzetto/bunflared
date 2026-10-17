//! What visitors leave in the feedback folder: notes with the elements they
//! point at, screenshots, and the session log of the live channel.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Deserialize;

use crate::clock::stamp;

pub const FOLDER: &str = "bunflared-feedback";
const MAX_PINNED: usize = 300;

#[derive(Debug, Clone)]
pub struct Feedback {
    pub sid: String,
    pub device: String,
    pub page: String,
    pub message: String,
    /// The note's Markdown file, and its screenshot.
    pub file: PathBuf,
    pub shot: Option<PathBuf>,
    /// In the order they were pointed at, numbered from 1.
    pub points: Vec<Element>,
}

impl Feedback {
    /// The remark, then a line per point when the points carry more than the
    /// file of before: what a reader sees in place of the file.
    pub fn text(&self) -> String {
        if plain(&self.points) {
            return self.message.clone();
        }
        let mut lines = Vec::new();
        if !self.message.is_empty() {
            lines.push(self.message.clone());
        }
        lines.extend(
            self.points
                .iter()
                .enumerate()
                .map(|(i, point)| point.line(i + 1)),
        );
        lines.join("\n")
    }
}

#[derive(Deserialize)]
pub struct Note {
    pub sid: String,
    pub page: String,
    pub message: String,
    screen: Option<String>,
    shot: Option<String>,
    #[serde(default)]
    pub points: Vec<Element>,
}

impl Note {
    /// A remark, or a comment under one of its points.
    pub fn written(&self) -> bool {
        !self.message.trim().is_empty() || self.points.iter().any(|p| !p.comment.trim().is_empty())
    }
}

/// An element a visitor pointed at, so whoever reads the note finds it, and
/// what they said about it.
#[derive(Debug, Clone, Deserialize)]
pub struct Element {
    pub selector: String,
    pub text: String,
    #[serde(default)]
    pub comment: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Element {
    /// `2. #buy "Buy now": make it green`, on one line.
    pub fn line(&self, number: usize) -> String {
        let mut line = format!("{number}. {}", self.selector);
        let quoted: String = self.text.chars().take(40).collect();
        if !quoted.is_empty() {
            line.push_str(&format!(" \"{quoted}\""));
        }
        let comment = self
            .comment
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !comment.is_empty() {
            line.push_str(&format!(": {comment}"));
        }
        line
    }

    fn markdown(&self) -> String {
        // Browser text goes inside backticks and quotes: keep it on one line
        // and unable to close them.
        let clean = |text: &str| -> String {
            text.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .replace(['`', '"'], "'")
                .chars()
                .take(MAX_PINNED)
                .collect()
        };
        let mut lines = format!("- Element: `{}`\n", clean(&self.selector));
        if !self.text.trim().is_empty() {
            lines.push_str(&format!("- Text: \"{}\"\n", clean(&self.text)));
        }
        lines.push_str(&format!(
            "- Position: {}, {} ({}×{}) from the top left of the page\n",
            self.x.round(),
            self.y.round(),
            self.w.round(),
            self.h.round()
        ));
        lines
    }
}

/// A note with at most one point and no comment under it keeps the file it
/// had before points existed: agents already read that one.
fn plain(points: &[Element]) -> bool {
    points.len() <= 1 && points.iter().all(|p| p.comment.trim().is_empty())
}

fn quote(text: &str) -> String {
    text.trim()
        .lines()
        .map(|line| format!("> {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The feedback folder, created with a `.gitignore` of its own so no project
/// ever commits a client's notes by accident.
pub fn folder_ready(folder: &Path) -> std::io::Result<()> {
    fs::create_dir_all(folder)?;
    let ignore = folder.join(".gitignore");
    if !ignore.exists() {
        fs::write(ignore, "# Client feedback collected by bunflared.\n*\n")?;
    }
    Ok(())
}

pub fn save_shot(folder: &Path, image: &[u8]) -> std::io::Result<String> {
    let extension = match image {
        [0x89, b'P', b'N', b'G', ..] => "png",
        [0xFF, 0xD8, 0xFF, ..] => "jpg",
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => "webp",
        _ => return Err(std::io::ErrorKind::InvalidData.into()),
    };
    folder_ready(folder)?;
    let name = format!("{}.{extension}", stamp());
    fs::write(folder.join(&name), image)?;
    Ok(name)
}

/// A share's session log, `session_<date>.md` next to the notes, opened on
/// its first line.
pub struct Transcript {
    folder: PathBuf,
    path: Mutex<Option<PathBuf>>,
}

impl Transcript {
    pub fn new(folder: PathBuf) -> Self {
        Self {
            folder,
            path: Mutex::default(),
        }
    }

    pub fn keep(&self, line: &str) {
        let mut opened = self.path.lock().unwrap();
        let path =
            opened.get_or_insert_with(|| self.folder.join(format!("session_{}.md", stamp())));
        let fresh = !path.exists();
        if folder_ready(&self.folder).is_err() {
            return;
        }
        let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
            return;
        };
        let now = chrono::Local::now();
        if fresh {
            let _ = writeln!(file, "# Live session, {}\n", now.format("%Y-%m-%d %H:%M"));
        }
        let _ = writeln!(file, "- {} {line}", now.format("%H:%M:%S"));
    }
}

/// Returns the note's file name, and its screenshot's.
pub fn save_note(
    folder: &Path,
    note: &Note,
    device: &str,
) -> std::io::Result<(String, Option<String>)> {
    folder_ready(folder)?;
    // Only a name this module gave out, never a path from the browser.
    let shot = note.shot.as_deref().filter(|name| {
        name.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            && !name.contains("..")
            && folder.join(name).is_file()
    });
    let mut text = format!(
        "# Feedback, {}\n\n- Page: `{}`\n- Device: {device}\n- Screen: {}\n- Visitor: {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        note.page,
        note.screen.as_deref().unwrap_or("unknown"),
        note.sid,
    );
    if plain(&note.points) {
        let pinned = note.points.first().map(Element::markdown);
        text.push_str(&format!(
            "{}\n{}\n",
            pinned.unwrap_or_default(),
            quote(&note.message)
        ));
    } else {
        for (index, point) in note.points.iter().enumerate() {
            text.push_str(&format!("\n## Point {}\n\n{}", index + 1, point.markdown()));
            if !point.comment.trim().is_empty() {
                text.push_str(&format!("\n{}\n", quote(&point.comment)));
            }
        }
        if !note.message.trim().is_empty() {
            text.push_str(&format!("\n## Overall\n\n{}\n", quote(&note.message)));
        }
    }
    if let Some(shot) = shot {
        let outlined = match note.points.len() {
            0 => "",
            1 => ", the element outlined",
            _ => ", the elements outlined and numbered",
        };
        text.push_str(&format!("\n![Screenshot{outlined}]({shot})\n"));
    }
    let name = format!("{}.md", stamp());
    fs::write(folder.join(&name), text)?;
    Ok((name, shot.map(str::to_string)))
}
