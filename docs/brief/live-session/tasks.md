---
type: tasks
slug: live-session
source_brief: docs/brief/live-session/brief.md
---

# Tasks: Live session, talk to the people on the link

## Relevant Files

- `src/widget.rs` - the `/_bunflared/` endpoints, where the live channel goes
- `src/widget.js` - the in-page widget: presence, feedback, screenshots
- `src/proxy.rs` - routes `/_bunflared/` and relays WebSocket upgrades
- `src/share.rs` - the events the dashboard receives
- `src/tui/mod.rs` - keys, state, toasts, achievements
- `src/tui/dashboard.rs` - panels, layout, QR code
- `src/tui/fx.rs` - the current hand-made particles and palette

## Tasks

Ordered. Each task closes the acceptance criteria it names. They build on
each other and can run in one `ship` pass.

## T01 - Live channel and driving the demo

Closes: AC1, AC2, AC3, AC4

- [x] each widget page keeps a reconnecting live connection to the dashboard
- [x] presence still works when that connection cannot open
- [x] visitors are selectable, `Tab` switches panels, `Esc` clears
- [x] `g` sends the selected visitor, or everyone, to a path
- [x] `R` reloads the selected visitor's page, or every page

## T02 - Chat both ways

Closes: AC5, AC6, AC7

- [x] `m` opens a message box where keys are text, not shortcuts
- [x] the message shows on the page as a bubble within a second
- [x] the visitor answers from the widget; the answer lands in a chat panel
- [x] the conversation is saved as a transcript in `bunflared-feedback/`

## T03 - Pointer radar

Closes: AC8, AC9

- [x] the radar shows the selected visitor's pointer on their viewport
- [x] nothing is sent while the pointer rests
- [x] the page shows a "live" indicator while its pointer is followed

## T04 - Reactions

Closes: AC10

- [ ] the widget offers reactions
- [ ] each reaction has its own dashboard animation and a counter
- [ ] the developer can send a reaction back

## T05 - Pinned feedback

Closes: AC11

- [ ] "Point at it" lets the visitor pick an element
- [ ] the note records selector, text excerpt and position
- [ ] the screenshot outlines the picked element

## T06 - Inspector and replay

Closes: AC12, AC13

- [ ] Enter on a request shows its details, bodies capped and binary named
- [ ] `p` replays a request and shows the new status
- [ ] requests over the capture limit say so and are not replayable

## T07 - Effects and QR polish

Closes: AC14, AC15

- [ ] tachyonfx works with ratatui 0.30, or the gap is reported
- [ ] entry, arrival, 5xx, disco and goodbye effects
- [ ] `--calm` and `NO_COLOR` switch every effect off
- [ ] `r` flashes the QR panel when it is on screen, opens the big QR otherwise

## T08 - Docs and demo

Closes: AC16

- [ ] README, `--help`, help overlay, `SKILL.md` and agents note updated
- [ ] JSON and `--detach` output unchanged
- [ ] the README GIF re-recorded with a chat exchange and a reaction
