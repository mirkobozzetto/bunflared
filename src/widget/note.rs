//! A visitor's note: a remark, the elements it points at with a comment each,
//! and the Markdown file it is saved as.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{folder_ready, stamp};

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

#[derive(Deserialize)]
pub(super) struct Note {
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

/// Returns the note's file name, and its screenshot's.
pub(super) fn save_note(
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
