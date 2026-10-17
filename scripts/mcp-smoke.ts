// `bunflared mcp` end to end, local mode only: a tiny app, one visitor that
// pings, talks over the live channel and leaves a note, and every tool.
// bun scripts/mcp-smoke.ts [binary], target/debug/bunflared by default.
import { spawnSync } from "bun";
import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const BINARY = resolve(process.argv[2] ?? "target/debug/bunflared");
const PROTOCOL = "2025-06-18";
const TOOLS = ["open", "list", "close", "visitors", "say", "go", "reload", "screenshot", "events", "requests", "request", "replay"];
const AGENT =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";
const DEVICE = "Mac · Chrome";
const SID = "smoke1";
const CLOCK = /^\d{2}:\d{2}:\d{2}$/;
const WAIT = 10;
const DEADLINE = 90_000;
const PNG = Uint8Array.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
const ECHOED = '{"n":1}';
const MISSING = 99999;
const PNG_BASE64 = "iVBORw0KGgo=";
const SOURCE = { component: "App", file: "src/App.tsx", line: 12, column: 5 };
const AT = "src/App.tsx:12:5 in App";

const app = Bun.serve({
  port: 0,
  async fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === "/") {
      return new Response("<!doctype html><title>smoke</title><body><button id=buy>Buy now</button></body>", {
        headers: { "content-type": "text/html; charset=utf-8" },
      });
    }
    if (path === "/echo") {
      return new Response(await request.text(), { headers: { "content-type": "application/json" } });
    }
    return new Response("missing", { status: 404 });
  },
});

const folder = mkdtempSync(join(tmpdir(), "bunflared-smoke-"));
const server = Bun.spawn([BINARY, "mcp"], { cwd: folder, stdin: "pipe", stdout: "pipe", stderr: "inherit" });
const watchdog = setTimeout(() => {
  console.log(`not ok - no end after ${DEADLINE / 1000}s`);
  server.kill();
  process.exit(1);
}, DEADLINE);

const pending = new Map<number, (message: any) => void>();
const channels: any[] = [];
let lastId = 0;

(async () => {
  const decoder = new TextDecoder();
  let buffer = "";
  for await (const chunk of server.stdout) {
    buffer += decoder.decode(chunk, { stream: true });
    for (let end = buffer.indexOf("\n"); end >= 0; end = buffer.indexOf("\n")) {
      const message = JSON.parse(buffer.slice(0, end));
      buffer = buffer.slice(end + 1);
      if (pending.has(message.id)) {
        pending.get(message.id)!(message);
        pending.delete(message.id);
      } else if (message.method === "notifications/claude/channel") {
        channels.push(message.params);
      }
    }
  }
})();

function rpc(method: string, params: object = {}): Promise<any> {
  const id = ++lastId;
  server.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
  server.stdin.flush();
  return new Promise((done) => pending.set(id, done));
}

async function tool(name: string, args: object = {}): Promise<{ error: boolean; text: string; value: any }> {
  const { result } = await rpc("tools/call", { name, arguments: args });
  const text: string = result.content[0].text;
  return { error: result.isError, text, value: result.isError ? undefined : JSON.parse(text) };
}

let passed = 0;
let failed = 0;
function check(label: string, ok: boolean, seen?: unknown) {
  if (ok) passed++;
  else failed++;
  console.log(`${ok ? "ok" : "not ok"} ${passed + failed} - ${label}`);
  if (!ok && seen !== undefined) console.log(`  seen: ${JSON.stringify(seen)}`);
}

// Every event the agent got, so waiting for one kind never loses another.
const log: any[] = [];
const taken = new Set<number>();
let cursor = 0;
async function event(kind: string): Promise<any> {
  const deadline = Date.now() + WAIT * 1000;
  for (;;) {
    const found = log.find((e) => e.kind === kind && !taken.has(e.seq));
    if (found) {
      taken.add(found.seq);
      return found;
    }
    if (Date.now() > deadline) return undefined;
    const { value } = await tool("events", { since: cursor, wait: WAIT });
    cursor = value.cursor;
    log.push(...value.events);
  }
}

async function eventually<T>(look: () => T | undefined): Promise<T | undefined> {
  const deadline = Date.now() + WAIT * 1000;
  for (;;) {
    const found = look();
    if (found !== undefined || Date.now() > deadline) return found;
    await Bun.sleep(50);
  }
}

function ls(): any[] {
  return JSON.parse(spawnSync([BINARY, "ls", "--json"]).stdout.toString());
}

const init = await rpc("initialize", { protocolVersion: PROTOCOL, capabilities: {}, clientInfo: { name: "smoke" } });
check("initialize names bunflared and keeps the asked revision",
  init.result.serverInfo.name === "bunflared" && init.result.protocolVersion === PROTOCOL, init.result);

const listed = await rpc("tools/list");
const names = listed.result.tools.map((t: any) => t.name);
check("tools/list gives the 12 tools", JSON.stringify(names) === JSON.stringify(TOOLS), names);

const early = await tool("visitors");
check("a share tool before open says to open one", early.error && early.text === "No share is open: open one first.", early.text);

const opened = await tool("open", { ports: [app.port] });
const share = opened.value;
check("open serves the app locally",
  !opened.error && /^http:\/\/127\.0\.0\.1:\d+$/.test(share.url) && share.local === true && share.routes["/"] === app.port,
  opened.text);

const shares = (await tool("list")).value;
check("list has the share", shares.length === 1 && shares[0].id === share.id, shares);

check("bunflared ls --json sees the share", ls().some((r) => r.id === share.id && r.url === share.url));

const headers = { "user-agent": AGENT };
const page = await fetch(share.url, { headers: { ...headers, accept: "text/html" } });
const html = await page.text();
check("the page carries the widget", page.status === 200 && html.includes('<script src="/_bunflared/widget.js" defer></script>'), html);

const script = await fetch(`${share.url}/_bunflared/widget.js`, { headers });
check("the widget script is served",
  script.status === 200 && script.headers.get("content-type")!.includes("javascript") && (await script.text()).length > 1000);

const ping = (gone: boolean) =>
  fetch(`${share.url}/_bunflared/ping`, {
    method: "POST",
    headers,
    body: JSON.stringify({ sid: SID, page: "/", visible: true, idle: 0, clicks: 1, gone }),
  });
await ping(false);
const arrived = await event("arrived");
check("a ping is an arrival",
  arrived?.visitor === SID && arrived.device === DEVICE && arrived.page === "/" && arrived.share === share.id && CLOCK.test(arrived.at),
  arrived);

const here = (await tool("visitors")).value;
check("visitors shows them active",
  here.length === 1 && here[0].visitor === SID && here[0].status === "active" && /^\d+s$/.test(here[0].here_for), here);

const socket = new WebSocket(`${share.url.replace("http", "ws")}/_bunflared/live?sid=${SID}`, { headers } as any);
const frames: any[] = [];
socket.onmessage = (message) => frames.push(JSON.parse(String(message.data)));
await new Promise((open) => (socket.onopen = open));
const frame = (type: string) => eventually(() => frames.find((f) => f.type === type));
// The next frame of a kind, each taken once.
const take = async (type: string) => {
  const found = await frame(type);
  if (found) frames.splice(frames.indexOf(found), 1);
  return found;
};
// The page's side of a look: the screenshot saved, then the answer on the channel.
async function answerLook(missing: boolean) {
  const asked = await take("look");
  const saved = await (await fetch(`${share.url}/_bunflared/shot`, { method: "POST", headers, body: PNG })).json();
  socket.send(JSON.stringify({ type: "shot", id: asked?.id, shot: saved.shot, title: "smoke", page: "/", screen: "800x600", text: "Buy now", missing }));
  return asked;
}

const said = await tool("say", { text: "hello from smoke", visitor: SID });
const bubble = await frame("chat");
check("say reaches the page", said.value?.pages === 1 && bubble?.text === "hello from smoke", [said.text, frames]);

const went = await tool("go", { path: "pricing" });
const go = await frame("go");
check("go sends the page to an absolute path", went.value?.pages === 1 && go?.path === "/pricing", frames);

await tool("reload", { visitor: SID });
check("reload reaches the page", (await frame("reload")) !== undefined, frames);

const mute = await tool("say", { text: "   " });
check("say without text is refused", mute.error && mute.text === "Nothing to say: pass the text.", mute.text);

socket.send(JSON.stringify({ type: "chat", text: "  hi   there ", page: "/" }));
const message = await event("message");
check("a visitor's message is an event", message?.text === "hi there" && message.device === DEVICE && message.visitor === SID, message);

const heard = await eventually(() => channels.find((c) => c.meta.kind === "message"));
check("the message arrives on the channel",
  heard?.content === "hi there" && heard.meta.share === share.id && heard.meta.visitor === SID, heard);

socket.send(JSON.stringify({ type: "react", emoji: "🔥", page: "/" }));
const reaction = await event("reaction");
check("a reaction is an event", reaction?.emoji === "🔥" && reaction.label === "on fire" && reaction.device === DEVICE, reaction);

const looking = rpc("tools/call", { name: "screenshot", arguments: {} });
const look = await answerLook(false);
const seen = (await looking).result;
const seenValue = JSON.parse(seen.content[0].text);
check("screenshot asks the page and hands back its image and what it shows",
  look?.reload === false && !seen.isError && seen.content[1]?.type === "image" && seen.content[1].data === PNG_BASE64 &&
    seenValue.title === "smoke" && seenValue.text === "Buy now" && seenValue.visitor === SID && seenValue.screenshot.endsWith(".png"),
  [look, seen]);

const reloading = rpc("tools/call", { name: "reload", arguments: { selector: "#buy" } });
const recheck = await answerLook(false);
const rechecked = (await reloading).result;
check("reload with a selector reloads and hands back the element's image",
  recheck?.reload === true && recheck.selector === "#buy" && rechecked.content[1]?.type === "image" &&
    JSON.parse(rechecked.content[0].text).found === true,
  [recheck, rechecked]);

socket.send(JSON.stringify({ type: "error", kind: "error", message: "Error: boom", source: "/src/App.tsx:3:9", stack: "Error: boom\n    at App", page: "/", count: 3 }));
const thrown = await event("error");
check("a page's error is an event", thrown?.message === "Error: boom" && thrown.type === "error" && thrown.count === 3 && thrown.source === "/src/App.tsx:3:9", thrown);
const heardError = await eventually(() => channels.find((c) => c.meta.kind === "error"));
check("the error arrives on the channel", heardError?.content === "Error: boom" && heardError.meta.count === 3, heardError);

const shot = await (await fetch(`${share.url}/_bunflared/shot`, { method: "POST", headers, body: PNG })).json();
check("a screenshot is saved", /^\d{4}-\d{2}-\d{2}_[\d.-]+\.png$/.test(shot.shot), shot);

const point = (selector: string, text: string, comment: string) => ({ selector, text, comment, x: 10, y: 20, w: 100, h: 40 });
const posted = await fetch(`${share.url}/_bunflared/feedback`, {
  method: "POST",
  headers,
  body: JSON.stringify({
    sid: SID,
    page: "/",
    message: "Looks good overall",
    screen: "1440×900",
    shot: shot.shot,
    points: [{ ...point("#buy", "Buy now", "make it green"), source: SOURCE }, point("h1", "Title", "bigger")],
  }),
});
const note = await event("note");
check("a note is an event with its points and screenshot",
  posted.status === 200 && note?.points.length === 2 && note.element.selector === "#buy" && note.screenshot.endsWith(shot.shot) &&
    note.points[0].source.file === SOURCE.file && note.points[0].source.line === SOURCE.line && note.points[1].source === null,
  note);

const markdown = note ? readFileSync(note.file, "utf8") : "";
check("the note file has both points and the screenshot",
  ["## Point 1", "## Point 2", `- Source: \`${AT}\``, "> make it green", "## Overall", `- Device: ${DEVICE}`,
    `![Screenshot, the elements outlined and numbered](${shot.shot})`].every((part) => markdown.includes(part)),
  markdown);

const noted = await eventually(() => channels.find((c) => c.meta.kind === "note"));
check("the note arrives on the channel",
  noted?.content.includes(`1. #buy "Buy now": make it green [${AT}]`) && noted.meta.element === "#buy" && noted.meta.source === AT,
  noted);

const feedback = join(folder, "bunflared-feedback");
const sessions = readdirSync(feedback).filter((name) => /^session_.*\.md$/.test(name));
const transcript = sessions.length === 1 ? readFileSync(join(feedback, sessions[0]), "utf8") : "";
check("the session log keeps the chat, the reaction and the note",
  [`# Live session, `, `**you → ${DEVICE}**: hello from smoke`, `**${DEVICE}** on \`/\`: hi there`,
    `**${DEVICE}** on \`/\` reacted 🔥`, `**${DEVICE}** on \`/\` left a note: [`].every((part) => transcript.includes(part)),
  [sessions, transcript]);

await fetch(`${share.url}/echo`, { method: "POST", headers: { ...headers, "content-type": "application/json" }, body: ECHOED });
const rows = (await tool("requests")).value;
check("requests lists the traffic, newest first",
  rows.length === 2 && rows[0].method === "POST" && rows[0].path === "/echo" && rows[1].path === "/" &&
    rows[1].status === 200 && rows[0].n > rows[1].n && rows[0].visitor === "127.0.0.1" && CLOCK.test(rows[0].at),
  rows);

const detail = (await tool("request", { n: rows[0].n })).value;
check("request shows headers and both bodies",
  detail?.replayable === true && detail.request.body.text === ECHOED && detail.response.body.text === ECHOED &&
    detail.request.headers.some((h: string) => h.startsWith("user-agent: ")),
  detail);

const replayed = (await tool("replay", { n: rows[0].n })).value;
check("replay sends it again", replayed?.before === 200 && replayed.status === 200, replayed);

const missing = await tool("request", { n: MISSING });
check("an unknown request is refused", missing.error && missing.text === `No request ${MISSING} in the last 200.`, missing.text);

await ping(true);
const left = await event("left");
check("a goodbye ping is a departure", left?.visitor === SID && left.device === DEVICE, left);

socket.close();
const closed = await tool("close", { share: share.id });
const after = (await tool("list")).value;
check("close stops the share and its record",
  closed.value?.closed === share.id && after.length === 0 && !ls().some((r) => r.id === share.id),
  [closed.text, after]);

server.stdin.end();
const code = await Promise.race([server.exited, Bun.sleep(5000).then(() => "still running")]);
check("the server exits 0 when stdin closes", code === 0, code);

clearTimeout(watchdog);
app.stop(true);
rmSync(folder, { recursive: true, force: true });
console.log(`${passed}/${passed + failed} passed`);
process.exit(failed === 0 ? 0 : 1);
