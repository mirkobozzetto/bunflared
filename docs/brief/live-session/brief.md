---
type: brief
slug: live-session
title: Live session, talk to the people on the link
status: ready
created: 2026-09-26
next_action: Turn the dashboard into a live session with visitors: chat, send them to a page, see their pointer, reactions, pinned feedback, request inspector and replay, richer animations
resume_cmd: /ship docs/brief/live-session
base: main
branch: feat/live-session
issues: "#2"
---

# Live session, talk to the people on the link

## Problem

bunflared already shows who is on the link, on which page, and collects
feedback notes with screenshots. But the conversation is one way and slow:
the developer cannot answer a note, point the client to the page that
changed, or see what the client is looking at, and has to switch to a phone
call or a chat app during a demo. Developers also lose time reproducing the
requests a visitor or a webhook sent. The dashboard animations are hand-made
and stop at particles, and `r` opens a second, redundant QR code when one is
already on screen.

## Users

- The developer running `bunf`, demoing an app in progress to a client.
- The client or teammate opening the link, on a phone or a laptop.
- Coding agents reading the notes and the chat afterwards to fix what came up.

## Goals

- Talk both ways between the dashboard and any open page, instantly.
- Drive the demo from the terminal: send a visitor to a page, reload
  everyone after a fix, see where their pointer is.
- Let visitors react and point at exactly what they mean.
- Inspect and replay the requests that went through the link.
- Make the dashboard more alive with composable effects, without costing
  CPU at rest.

## Acceptance criteria

### Live channel

- AC1 Each page carrying the widget holds a live connection to the dashboard
  that reconnects by itself after a drop; everything below reaches the page
  within one second through the tunnel. When the connection cannot open,
  presence keeps working as today.

### Drive the demo

- AC2 In the visitors panel, the arrow keys select a visitor, `Tab` moves
  focus between the visitors panel and the request log, and the selection is
  visible. `Esc` clears it. With no selection, commands target every
  visitor.
- AC3 `g` asks for a path (with the pages already seen offered first) and
  sends the selected visitor, or everyone, to that page.
- AC4 `R` reloads every open page, or the selected visitor's, after a fix.

### Chat

- AC5 `m` opens a message box in the dashboard; while it is open, keys are
  typed text, never shortcuts. Enter sends the message to the selected
  visitor or to everyone; it appears on their page as a bubble from the
  widget.
- AC6 The visitor answers from the widget; the answer shows in a chat panel
  of the dashboard with the visitor's device and page, and as a toast.
- AC7 The conversation is kept as a Markdown transcript in
  `bunflared-feedback/`, next to the notes.

### See what they see

- AC8 A radar panel shows the selected visitor's pointer on a scaled outline
  of their viewport, updated several times per second while it moves, and
  nothing is sent while the pointer rests.
- AC9 While the dashboard follows a visitor's pointer, the page shows a small
  "live" indicator, so the visitor knows.

### Reactions

- AC10 The widget offers a few reactions (for example 👍 🔥 😍 😕). Each one
  shows in the dashboard with its own animation (confetti, fireworks,
  hearts, a worried bunny) and a counter; the developer can send one back.

### Pinned feedback

- AC11 In the feedback panel, "Point at it" lets the visitor click an
  element of the page. The note then records that element (a CSS selector, a
  text excerpt, its position) and the screenshot shows it outlined, so an
  agent reading the note knows exactly what to change.

### Inspector and replay

- AC12 Enter on a request in the log opens its details: method, path,
  status, timing, request and response headers, and the start of text bodies
  (up to a stated limit; binary bodies are named, not shown).
- AC13 `p` on a request replays it to the local server, and the new status
  shows next to the original. Requests whose body exceeded the capture limit
  say so and cannot be replayed.

### Animations and polish

- AC14 Dashboard effects built with tachyonfx: panels materialize on entry,
  a color sweep runs over the visitors panel when someone arrives, the
  request log glitches briefly on a 5xx, disco mode becomes a hue shift over
  the whole screen, and the dashboard dissolves before the goodbye.
  `--calm` and `NO_COLOR` turn them all off.
- AC15 `r` opens the big QR code only when the QR panel is not on screen;
  when it is, `r` makes the panel flash instead of opening a duplicate.

### Docs

- AC16 README, `--help`, the help overlay, `SKILL.md` and the agents note
  describe the new keys, the chat transcript and the pointer indicator.
  JSON and `--detach` output are unchanged.

## Success metrics

- A chat message typed in the dashboard shows on a phone opened on the link
  in under one second.
- The dashboard at rest stays under 5 % of one CPU core with effects on.
- The README GIF is re-recorded and shows a chat exchange and a reaction.
- On a page that forbids external scripts (strict CSP), the widget degrades
  without breaking the page.

## Out-of-scope

- Voice or video, screen sharing.
- Showing the developer's own pointer on the visitor's page.
- Several dashboards on one share, or any account or login.
- Editing a request before replaying it.
- A password on the link.

## Constraints and assumptions

- WebSockets cross Cloudflare quick tunnels (verified in this project: Vite
  hot reload works through the link); Server-Sent Events do not
  ([Cloudflare docs](https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/do-more-with-tunnels/trycloudflare/)).
  The live channel is a WebSocket served by bunflared under `/_bunflared/`.
- Keys already bound: `c o r f ? q b a`, arrows (Konami code) and the typed
  word `yum`. New keys must not collide with them.
- tachyonfx is the effects library of the ratatui ecosystem, with 50+
  composable effects ([tachyonfx](https://github.com/ratatui/tachyonfx));
  its compatibility with ratatui 0.30 is to be checked before use.
- Ideas borrowed from existing tools: follow mode and live cursors
  ([Magnyte Preview](https://magnytepreview.magnytesolution.com/)), comments
  pinned on the UI ([Vercel Comments](https://vercel.com/docs/comments)),
  inspect and replay ([ngrok](https://ngrok.com/docs/share-localhost/inspection)).
- Visitors must be able to tell they are watched: the pointer indicator and
  the existing README note.

## Boundary

Owns:
- `src/`
- `docs/demo/`
- `README.md`, `skills/bunflared/`
- `Cargo.toml`, `Cargo.lock`

Must not touch:
- `.github/`, `dist-workspace.toml`
