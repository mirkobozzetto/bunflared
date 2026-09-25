---
type: tasks
slug: bunflared
source_brief: docs/brief/bunflared/brief.md
---

# Tasks: bunflared

## Relevant Files

- `docs/brief/bunflared/share.ts` - the working prototype, reference for
  routing, rewriting, DNS wait and error messages

## Tasks

Ordered. Each task closes the acceptance criteria it names.

## T01 - Share parity with the prototype, plus WebSocket

Closes: AC1, AC2, AC3, AC4, AC5, AC6, AC7, AC8

- [ ] `/` and `/_port/<port>` reach the right local port
- [ ] localhost URLs are rewritten in text bodies and `Location`
- [ ] local servers receive `Host: localhost:<port>`
- [ ] WebSocket upgrades are relayed; Vite hot reload works through the link
- [ ] the link is announced after DNS resolves and copied to the clipboard
- [ ] each startup failure has its message, its fix and its exit code
- [ ] a dead port answers viewers with the bunny 502 page
- [ ] quitting leaves no `cloudflared` process and a clean terminal

## T02 - Agent mode

Closes: AC21, AC22, AC23, AC24, AC25

- [ ] non-terminal stdout or `--json` prints one JSON line, no ANSI
- [ ] `--detach` returns when ready and the share outlives the caller
- [ ] `ls` and `ls --json` list live shares and drop dead ones
- [ ] `down <id>` and `down --all` stop shares and their `cloudflared`
- [ ] `--help` covers share, list and stop

## T03 - The show: scenes from boot to goodbye

Closes: AC9, AC10, AC11, AC12, AC18, AC19, AC20

- [ ] animated boot logo and bunny, skippable, at most 1.5 s
- [ ] animated preflight per port
- [ ] digging scene with particles, a real-progress depth meter and jokes
- [ ] launch explosion with the big link and the clipboard toast
- [ ] goodbye animation and a recap card left in the scrollback
- [ ] live resize and a compact layout for small terminals
- [ ] `--calm` and `NO_COLOR`

## T04 - Live dashboard

Closes: AC13, AC17

- [ ] scannable QR code of the link
- [ ] port heartbeats with a visible down state
- [ ] traffic lane of request sprites colored by status class
- [ ] requests-per-second sparkline, request log, visitors, totals
- [ ] keys `c`, `o`, `?`, `q`, the help overlay and the footer

## T05 - Personality

Closes: AC14, AC15, AC16

- [ ] bunny moods: asleep, hopping, panicking
- [ ] at least five animated achievements
- [ ] Konami disco mode, `f` fireworks, one hidden secret

## T06 - Distribution

Closes: AC26, AC27

- [ ] `skills/bunflared/SKILL.md` for coding agents
- [ ] README with install, usage and the agent section
- [ ] `cargo install --git` produces a working binary
