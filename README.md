# bunflared

![bunflared sharing a frontend and its API: digging the tunnel, going live, live traffic, QR code, session recap](https://raw.githubusercontent.com/mirkobozzetto/bunflared/main/docs/demo/dashboard.gif)

`bunflared 5173 3000` puts your local app on a temporary
`https://<words>.trycloudflare.com` link, through a Cloudflare quick tunnel.
No account, no deploy. Send the link to a client, open it on your phone,
close the terminal and it is gone.

It shares several ports behind one link, so a frontend and its API travel
together: the first port is served at `/`, the others under `/_port/<port>`,
and `http://localhost:<port>` inside your pages and scripts is rewritten to
match. WebSockets pass through, so hot reload keeps working for the person on
the other end.

And it is a show. The screen catches fire while the tunnel opens, meteors
crash into the flames, a firefighter bunny tries his best, and a phoenix rises
out of the fire carrying your link. Then every request flies across a traffic
lane, the bunny naps when nobody visits and panics on a 5xx. There are
achievements. There are secrets.

And it is a live session with the people on the link: chat with them, send
them to the page you just fixed, see where their pointer is, get their
reactions and their notes pinned on the exact element they mean.

## Install

macOS and Linux:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/mirkobozzetto/bunflared/releases/latest/download/bunflared-installer.sh | sh
```

Windows (PowerShell):

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/mirkobozzetto/bunflared/releases/latest/download/bunflared-installer.ps1 | iex"
```

That is all: no Rust, no `cloudflared` to install first. You get `bunflared`
and its short name `bunf`. On the first share, bunflared downloads
Cloudflare's own `cloudflared` if it is not already installed.

With Rust: `cargo install bunflared`.

Then, once, so your coding agents know to reach for it when you ask to share
something:

```sh
bunflared agents
```

## Use

```sh
bunf 5173                       # share one app
bunf 5173 3000                  # app at /, its API at /_port/3000
bunf 5173 --calm                # same dashboard, no animations
```

`bunf` and `bunflared` are the same command.

In the dashboard:

| Key | Does |
| --- | --- |
| `c` | copy the link (it is already in your clipboard) |
| `o` | open it in your browser |
| `r` | big QR code, for phones (it flashes when already on screen) |
| `f` | fireworks |
| `↑` `↓` | pick a visitor; commands then go to them only |
| `Tab` | switch between the visitors and the requests |
| `Esc` | pick nobody: commands go to everyone again |
| `m` | message the visitor, or everyone |
| `g` | send them to a page (the pages already seen are offered) |
| `R` | reload their page, after a fix |
| `e` | send them a reaction |
| `Enter` | on a request: its headers and bodies |
| `p` | replay the picked request to your local server |
| `?` | help |
| `q` | stop sharing |

Colors follow your terminal: bunflared asks it for its background and picks a
light or dark palette. `--theme light` or `--theme dark` forces one. `NO_COLOR`
is respected. When a shared port stops answering, visitors get a
small bunny page that retries by itself.

## Feedback from the people you share with

Every shared page gets a small **✎ Feedback** button. Your client writes a
remark, a screenshot of the page is attached (or they paste their own), and it
lands in `bunflared-feedback/`, in the folder you ran bunflared from: one
Markdown file per note, the image next to it. With **Point at it** they click
the element they mean first: the note records its CSS selector, its text and
its position, and the screenshot shows it outlined. That folder ignores itself
in git, so client notes never end up in a commit. Hand it to your coding
agent: "read the feedback and fix what they found".

Meanwhile the dashboard shows who is on which page, whether their tab is in
front, how long they have been idle and how many times they clicked.

## Live session

Each page holds a live connection to the dashboard, and reconnects by itself.

- **Chat**: `m` opens a message box. The message pops up on their page, they
  answer from it, and the answer lands in the chat panel and as a toast.
- **Drive the demo**: `g` sends them to a page, `R` reloads it.
- **Radar**: pick a visitor and the radar draws their pointer on an outline of
  their screen. Their page shows a small "Live" pill while it is followed, and
  nothing is sent while the pointer rests.
- **Reactions**: 👍 🔥 😍 😕 next to the feedback button. Each one has its own
  show in the dashboard, confetti, fireworks, hearts or a worried bunny, and a
  counter. `e` sends one back.
- **Inspector**: `Tab` to the requests, pick one, `Enter` shows its headers
  and the start of its bodies, `p` replays it to your local server and shows
  the new status next to the old one.

The chat, the reactions and a link to every note are kept in one
`session_<date>.md` next to the notes, so the whole conversation is there for
you or your coding agent afterwards. The widget adapts to phones and follows
the page's light or dark look.

Tell the people you share with that their visit is followed. `--no-widget`
leaves the pages untouched. The screenshot library is loaded from jsDelivr
only when someone sends a screenshot.

## For AI agents

Outside a terminal, or with `--json`, there is no dashboard: one JSON line on
stdout once the link works, one on stderr if it fails, and a meaningful exit
code. `--detach` returns as soon as the link works and keeps sharing in the
background.

```sh
bunflared 5173 3000 --detach    # {"id":"4242","url":"https://...","routes":{...},...}
bunflared ls --json
bunflared down 4242             # or: bunflared down --all
```

The feedback button stays on the pages of a detached share: you can leave
notes on your app while your agent works, and it reads them in
`bunflared-feedback/`.

### Teach your agents

`bunflared agents` writes a short note into the global instructions of
every coding agent it finds, so any session knows the command without being
told:

| Agent | Where the note goes |
| --- | --- |
| Claude Code | `~/.claude/rules/bunflared.md` |
| omp | `~/.omp/agent/rules/bunflared.md` |
| pi | `~/.pi/agent/AGENTS.md`, in a marked block |
| prime-agent | `~/.prime/agent/AGENTS.md`, in a marked block |
| Codex | `~/.codex/AGENTS.md`, in a marked block |

Run it again after an update to refresh the note, `bunflared agents --remove`
to take it back out. For any other agent, paste the output of
`bunflared agents --print` into its rules.

Prefer skills? `skills/bunflared/SKILL.md` follows the Agent Skills format, and
[skills](https://github.com/vercel-labs/skills) installs it into Claude Code,
Codex, Cursor, pi and many more in one line:

```sh
npx skills add mirkobozzetto/bunflared -g
```

## Limits

- Anyone with the link reaches your dev server. The random name is the only
  protection.
- Quick tunnels allow 200 requests in flight and do not carry Server-Sent
  Events ([Cloudflare docs](https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/do-more-with-tunnels/trycloudflare/)).
- Quick tunnels refuse to start while `~/.cloudflared/config.yaml` exists.
- Cloudflare hands out a limited number of new quick links in a short time.
  Past it, bunflared says so: wait a few minutes.
- The inspector keeps the first 32 KiB of each body, in memory, for the last
  200 requests. A request with a bigger body cannot be replayed.
- Tested by hand on macOS. The Linux and Windows builds compile in CI but
  have not been tried by hand yet.

## License

MIT
