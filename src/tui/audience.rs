use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use super::{Ask, Prompt, plural};
use crate::clock;
use crate::event::{Pointer, Reacted, Said};
use crate::live::{Command, Hub, REACTIONS};
use crate::notes::Feedback;
use crate::visitors::{Presence, Roster};

const PAGES_KEEP: usize = 20;
const SUGGESTIONS: usize = 5;
const CHAT_KEEP: usize = 60;

/// A line of the chat panel, sent or received.
pub struct Message {
    pub mine: bool,
    /// The visitor's device, or who a message of ours went to.
    pub who: String,
    pub page: String,
    pub text: String,
    pub clock: String,
}

/// The visitors on the page: who is there, what they say, what we send them.
pub struct Audience {
    pub roster: Roster,
    /// The followed visitors' pointers, and when they last moved.
    pub pointers: HashMap<String, (Pointer, Instant)>,
    pub chat: VecDeque<Message>,
    /// Pages the visitors were on, the most recent first.
    pages: Vec<String>,
    pub reactions: [u32; REACTIONS.len()],
    pub feedback: u32,
    /// The visitor that commands go to; everyone when `None`.
    pub selected: Option<String>,
    hub: Arc<Hub>,
}

impl Audience {
    pub fn new(hub: Arc<Hub>) -> Self {
        Self {
            roster: Roster::default(),
            pointers: HashMap::new(),
            chat: VecDeque::new(),
            pages: Vec::new(),
            reactions: [0; REACTIONS.len()],
            feedback: 0,
            selected: None,
            hub,
        }
    }

    /// True when the visitor just arrived.
    pub fn presence(&mut self, presence: Presence, now: Instant) -> bool {
        self.remember(&presence.page);
        self.roster.update(presence, now)
    }

    pub fn pointer(&mut self, pointer: Pointer, now: Instant) {
        if self.roster.get(&pointer.sid).is_some() {
            self.pointers.insert(pointer.sid.clone(), (pointer, now));
        }
    }

    /// Joins the chat, and returns the toast that announces it.
    pub fn note(&mut self, note: Feedback) -> (String, String) {
        self.feedback += 1;
        let excerpt: String = note.text().replace('\n', " · ").chars().take(40).collect();
        let title = format!("✎ {} on {}", note.device, note.page);
        // A visitor often answers in the note field: it joins the chat
        // too, a line per element it points at, and whether a
        // screenshot came.
        let mut text = format!("✎ {}", note.message).trim_end().to_string();
        for (index, point) in note.points.iter().enumerate() {
            text.push_str(&format!("\n📍 {}", point.line(index + 1)));
        }
        if note.shot.is_some() {
            text.push_str("\n📷 screenshot");
        }
        self.message(Message {
            mine: false,
            who: note.device,
            page: note.page,
            text,
            clock: clock::clock(),
        });
        (title, excerpt)
    }

    /// Joins the chat, and returns the toast that announces it.
    pub fn said(&mut self, said: Said) -> (String, String) {
        let excerpt: String = said.text.chars().take(40).collect();
        let title = format!("✉ {} on {}", said.device, said.page);
        self.message(Message {
            mine: false,
            who: said.device,
            page: said.page,
            text: said.text,
            clock: clock::clock(),
        });
        (title, excerpt)
    }

    /// Counts a reaction, and returns the toast that announces it.
    pub fn reacted(&mut self, reaction: &Reacted) -> (String, &'static str) {
        self.reactions[reaction.kind] += 1;
        let (emoji, label) = REACTIONS[reaction.kind];
        (format!("{emoji} from {}", reaction.device), label)
    }

    /// The selected visitor is the one whose pointer the radar follows.
    pub fn select(&mut self, sid: Option<String>) {
        self.hub.follow(sid.as_deref());
        self.selected = sid;
    }

    /// Returns how many pages got it.
    pub fn send(&self, command: &Command) -> usize {
        self.hub.send(self.selected.as_deref(), command)
    }

    pub fn react(&self, kind: usize) -> usize {
        self.hub
            .react(self.selected.as_deref(), &self.target(), REACTIONS[kind].0)
    }

    /// A line of ours joins the chat once a page got it.
    pub fn say(&mut self, text: &str) -> usize {
        let to = self.target();
        let reached = self.hub.say(self.selected.as_deref(), &to, text);
        if reached > 0 {
            self.message(Message {
                mine: true,
                who: to,
                page: String::new(),
                text: text.to_string(),
                clock: clock::clock(),
            });
        }
        reached
    }

    /// The pages offered under the go box, filtered by what is typed, or
    /// the reactions to pick from.
    pub fn suggestions(&self, prompt: &Prompt) -> Vec<String> {
        match prompt.ask {
            Ask::Chat => return Vec::new(),
            Ask::React => {
                return REACTIONS
                    .iter()
                    .enumerate()
                    .map(|(i, (emoji, label))| format!("{}  {emoji}  {label}", i + 1))
                    .collect();
            }
            Ask::Go => {}
        }
        let text = prompt.text.trim();
        self.pages
            .iter()
            .filter(|page| page.contains(text))
            .take(SUGGESTIONS)
            .cloned()
            .collect()
    }

    /// Who commands go to, for titles and toasts.
    pub fn target(&self) -> String {
        self.selected
            .as_ref()
            .and_then(|sid| self.roster.get(sid))
            .map_or_else(|| "everyone".into(), |v| v.presence.device.clone())
    }

    /// What a command's toast says about who got it.
    pub fn reached(&self, pages: usize) -> String {
        match pages {
            0 => "No page is connected live.".to_string(),
            n => format!("{}, {}.", self.target(), plural(n, "page")),
        }
    }

    fn remember(&mut self, page: &str) {
        self.pages.retain(|seen| seen != page);
        self.pages.insert(0, page.to_string());
        self.pages.truncate(PAGES_KEEP);
    }

    fn message(&mut self, message: Message) {
        self.chat.push_back(message);
        if self.chat.len() > CHAT_KEEP {
            self.chat.pop_front();
        }
    }
}
