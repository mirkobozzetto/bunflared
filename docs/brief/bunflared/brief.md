---
type: brief
slug: bunflared
title: bunflared, share localhost with a public link and a ridiculous amount of fun
status: shipped
shipped_at: 2026-09-25T13:05:00+02:00
created: 2026-09-25
next_action: Ship a single Rust binary that shares local ports on a trycloudflare link, with an animated TUI for humans and a JSON mode for agents
resume_cmd: /ship docs/brief/bunflared
base: main
branch: feat/bunflared
---

# bunflared, share localhost with a public link and a ridiculous amount of fun

## Problem

Showing a work-in-progress app to someone else means deploying it or creating
an account somewhere. The prototype `share.ts` (kept next to this brief)
already solves the core: one command puts a frontend and its API on one
temporary public https address through a Cloudflare quick tunnel, no account.

It stops short in four ways:

- WebSocket upgrades are refused, so Vite hot reload does not work through
  the link and viewers have to reload by hand.
- The terminal shows a static block of text: no sense of who is visiting,
  whether the ports are still alive, or that anything is happening at all.
- AI coding agents cannot use it cleanly: it never returns control, prints
  human prose, and leaves no way to list or stop a share later.
- It needs Bun installed.

## Users

- The developer (Mirko first) who wants a link to show a local app to a
  client, a colleague or a phone, and wants that moment to feel great.
- The viewers who open the link, often on a phone.
- AI coding agents (Claude Code, Codex, ...) that need to hand a working URL
  to their user without blocking their own shell.

## Goals

- One command, one public https link for one or several local ports.
- Everything `share.ts` does, plus live reload through the link.
- A terminal experience that is absurdly fun: an animated bunny, a tunnel
  being dug, confetti, a live traffic show, achievements, easter eggs. The
  name reads as "bun + flare": the mascot is a bunny, the tunnel is its
  rabbit hole.
- A quiet, machine-readable mode an agent can drive in one command.
- A single binary with no runtime to install besides `cloudflared`.

## Acceptance criteria

### Sharing

- AC1 `bunflared 5173 3000`, with both ports answering, gives a public https
  `trycloudflare.com` link. `/` serves port 5173; `/_port/3000/...` serves
  port 3000.
- AC2 In text responses (HTML, JavaScript, CSS, JSON, plain text) and in
  `Location` headers, `http://localhost:<shared port>` and
  `http://127.0.0.1:<shared port>` are rewritten to the matching public
  path. Other bodies pass through byte for byte.
- AC3 The local servers see `Host: localhost:<port>`, so a Vite dev server
  does not reject the request as an unknown host.
- AC4 WebSocket upgrades are relayed in both directions: editing a file of
  a shared Vite app updates the page open on another device, no reload.
- AC5 The link is announced only once Cloudflare's public DNS resolves it,
  and it is copied to the clipboard when a clipboard tool exists.
- AC6 Each startup failure names its cause and its fix, with its own exit
  code: `cloudflared` missing (install command), a port not answering
  (which port), `~/.cloudflared/config.yaml` present (quick tunnels refuse
  to start with it), invalid arguments.
- AC7 When a shared port stops answering mid-session, viewers get a small
  friendly error page (an ASCII bunny looking for its carrot) with a 503,
  not a browser error. Not a 502: Cloudflare replaces an origin's 502 page
  with its own.
- AC8 Quitting (q, Ctrl-C, SIGTERM) stops the proxy and `cloudflared`. No
  `cloudflared` process survives, and the terminal is restored even after a
  crash.

### The show (interactive terminal)

- AC9 Boot: an animated color-sweeping logo and the bunny, at most 1.5 s,
  skipped by any key.
- AC10 Preflight: each port is checked on screen with its own animated
  state, and a failed port is shown as such before exiting.
- AC11 Digging: while the tunnel and DNS are not ready, the bunny digs with
  flying dirt particles. A depth meter follows real progress (tunnel up,
  each DNS attempt), and a rotating line of jokes keeps the wait alive.
- AC12 Launch: the bunny bursts out of the hole in a confetti or fireworks
  explosion, the link appears big, a toast confirms the clipboard copy.
- AC13 Dashboard: the link and uptime; a QR code of the link that a phone
  camera can scan from a normal terminal; each port with an animated
  heartbeat and a clear "down" state; a traffic lane where every request
  travels as a sprite colored by status class; a requests-per-second
  sparkline; a request log (method, path, status, duration, port); the
  number of distinct visitors; totals.
- AC14 The bunny has moods: it falls asleep after 60 s without traffic,
  hops when requests arrive, panics on a 5xx.
- AC15 At least five achievements pop as animated toasts, for example first
  visitor, 100 requests, first 5xx survived, one hour live, a response
  under 10 ms.
- AC16 Easter eggs: the Konami code switches to a disco palette, `f`
  launches fireworks, and at least one more secret is left for users to
  find.
- AC17 Keys: `c` copies the link, `o` opens it in the browser, `?` shows a
  help overlay, `q` quits. A footer lists them.
- AC18 Goodbye: the tunnel collapses, the bunny waves, and a recap card
  (duration, requests, visitors, errors, achievements) stays in the
  scrollback after exit. It lasts at most 2 s; a second Ctrl-C exits at
  once.
- AC19 Resizing is handled live. Below the minimum size, a compact layout
  still shows the link and the keys.
- AC20 `--calm`, or `NO_COLOR` set, turns animations off (and colors, for
  `NO_COLOR`) while keeping the same information.

### Agents

- AC21 When stdout is not a terminal, or with `--json`, there is no TUI and
  no ANSI escape: one JSON line on stdout once the link is ready
  (`id`, `url`, `routes`, `pid`); failures are one JSON line on stderr
  plus the exit code.
- AC22 `--detach` returns as soon as the link is ready, with that JSON
  line, and the share keeps running after the calling shell exits.
- AC23 `bunflared ls` (and `ls --json`) lists live shares with id, link,
  ports and uptime; entries whose process died are dropped.
- AC24 `bunflared down <id>` and `bunflared down --all` stop shares and
  their `cloudflared`.
- AC25 `bunflared --help` is enough on its own for an agent to share,
  list and stop.
- AC26 The repository ships `skills/bunflared/SKILL.md` for coding agents:
  when to use it, the commands, the JSON shape, the limits (no SSE, 200
  in-flight requests, public exposure) and how to stop.

### Install

- AC27 `cargo install --git https://github.com/mirkobozzetto/bunflared`
  installs a working `bunflared`; the README shows install, usage and the
  agent section.

## Success metrics

- Time to link: no more than about one second on top of `cloudflared`
  registration and DNS propagation.
- The dashboard at rest stays under 5 % of one CPU core.
- Zero `cloudflared` orphans (`pgrep cloudflared`) after `q`, Ctrl-C,
  SIGTERM, `down`, and a forced panic.
- Claude Code, given only `SKILL.md`, shares a running Vite app, returns
  the link in one command, then stops it.
- Mirko shows it to someone and they laugh at the bunny.

## Out-of-scope

- Cloudflare accounts, named tunnels, custom domains.
- Server-Sent Events through the link: quick tunnels do not support them.
  Documented, not worked around.
- Passwords or any access control on the link.
- Config files, Windows support, Homebrew tap, prebuilt releases.
- Telemetry, sound, country flags, request inspection or replay.
- An MCP server.

## Constraints and assumptions

- Rust stable, a single binary, `ratatui` for the terminal UI (decided).
- `cloudflared` is installed by the user. Quick tunnels cap in-flight
  requests at 200 (then `429`), do not support SSE, and do not start when
  `~/.cloudflared/config.yaml` exists
  ([Cloudflare docs](https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/do-more-with-tunnels/trycloudflare/)).
- macOS first; Linux should work. Clipboard through `pbcopy` on macOS,
  best effort elsewhere.
- Distinct visitors come from the client address header the tunnel
  forwards; to verify during implementation, with a fallback on the peer
  connection.
- Anyone with the link reaches the dev server. The random subdomain is the
  only protection; the TUI and `SKILL.md` say so in one line.
- `share.ts` in this folder is the behavioral reference for AC1 to AC6.

## Boundary

Owns:
- `src/`, `Cargo.toml`, `Cargo.lock`
- `skills/bunflared/`, `README.md`
- `~/.local/state/bunflared/`

Must not touch:
- `~/.cloudflared/` (read only, to detect `config.yaml`)
- the code and config of the apps being shared
