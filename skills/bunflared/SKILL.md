---
name: bunflared
description: Share a local dev server on a temporary public https link through a Cloudflare quick tunnel, no account needed. Use when the user wants to show a running local app to someone, open it on their phone, or get a public URL for localhost (frontend and API ports together).
---

# bunflared

Puts local ports on a temporary `https://<words>.trycloudflare.com` link.
When `cloudflared` is not installed, bunflared downloads Cloudflare's build on
the first run.

## Share

From an agent, always pass `--detach`. It returns once the link works and
keeps sharing in the background:

    bunflared 5173 3000 --detach

- The first port is served at `/`, every other port under `/_port/<port>`.
- `http://localhost:<port>` inside HTML, JS, CSS and JSON responses is
  rewritten to the public path, so a frontend calling its API on localhost
  keeps working. WebSockets pass through, so hot reload works.
- It usually takes under 10 seconds, up to 40 when DNS is slow: the time for
  the new name to exist.

On success it prints one JSON line:

    {"id":"4242","pid":4242,"tunnel_pid":4243,"url":"https://....trycloudflare.com","routes":{"/":5173,"/_port/3000":3000},"started_at":1790000000}

Give the user the `url`. Keep the `id` to stop it later.

Without `--detach` the command never returns until stopped: in a terminal it
opens an interactive dashboard, elsewhere it prints the JSON line and waits.

## Feedback

Each shared page carries a feedback button. Notes from visitors are saved in
`bunflared-feedback/` in the folder bunflared runs from: one Markdown file per
note (page, device, message) with its screenshot next to it. A note made with
"Point at it" also names the element: CSS selector, text, position, and the
screenshot outlines it. `session_<date>.md` there keeps the chat, the
reactions and a link to each note, in order. Read them when the user asks what
their client thought, or to find what to fix. The folder is git-ignored by
itself. When the user follows a visitor's pointer from the dashboard, that
visitor's page shows a "Live" pill. `--no-widget` shares the pages untouched.

The user can leave notes on their own app while you keep it shared with
`--detach`: read the folder when they say they left feedback.

## List and stop

    bunflared ls --json      # JSON array of live shares, same objects
    bunflared down <id>      # stop one share
    bunflared down --all     # stop every share

Stop the share when the user is done with it.

## Failures

One JSON line on stderr, `{"error":"...","code":N}`, with the same exit code:

| Code | Meaning | What to do |
| --- | --- | --- |
| 1 | the tunnel closed | share again |
| 2 | bad arguments | fix the command |
| 3 | `cloudflared` missing, download failed | install it, e.g. `brew install cloudflared` |
| 4 | a port is not answering | start the app first |
| 5 | `~/.cloudflared/config.yaml` exists | ask the user before renaming it |
| 6 | the tunnel failed to start | retry; when the message names Cloudflare's limit on new links, wait a few minutes first |

## Tell the user

- Anyone with the link reaches the app. There is no password.
- Quick tunnels do not carry Server-Sent Events and allow 200 requests in
  flight at most.
