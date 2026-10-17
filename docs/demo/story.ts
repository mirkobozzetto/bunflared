// Films the README clips and captures. A recording plays one story on its own
// share, filming the browser and the terminals at the same time, then cuts it
// into clips that go back and forth between them.
// Needs ttyd, ffmpeg, gifsicle and chafa. From docs/demo, after
// `bun install && bun install --cwd shop`, with `bun shop/apps.ts` running:
//   bun story.ts <recording>   live, inspect, agent or password
//   bun story.ts <folder>      cuts again from a kept recording
// live films dashboard.gif and drive.gif on a public link; inspect films
// inspect.gif; agent films edit.gif and agent.gif and takes note.png; password
// takes password.png on a public link.
import { type ChildProcess, spawn, spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, statSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { type Browser, type BrowserContext, chromium, devices, type Locator, type Page } from "playwright";

const SIZE = { width: 1280, height: 760 };
const BAND = 56;
const GIF_WIDTH = 1200;
const FPS = 15;
const COLORS = 256;
const LOSSY = 40;
const PORTS = { term: 7681, shell: 7682, agent: 7683 };
const SHOP = 5174;
const API = 3000;
// Long enough to read what just happened, after each gesture.
const READ_MS = 2500;
const TYPE_MS = 70;
// A pointer at a person's pace: about 600 px a second.
const GLIDE_PX = 10;
const FRAME_MS = 16;
const PHONE = devices["iPhone 15"];
// A plain desktop Chrome: the dashboard names the visitor's browser from it.
const USER_AGENT =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";
const DRACULA = {
  background: "#282a36", foreground: "#f8f8f2", cursor: "#f8f8f2",
  black: "#21222c", red: "#ff5555", green: "#50fa7b", yellow: "#f1fa8c",
  blue: "#bd93f9", magenta: "#ff79c6", cyan: "#8be9fd", white: "#f8f8f2",
};
const MESSAGE = "Hi! The checkout is ready, have a look";
const ANSWER = "Nice! But the pay button says NaN €";
const BAD_CODE = "SPRING24";
const ASK_FILE = "ask";

type Side = "term" | "web" | "shell" | "agent";
const SIDES: Record<Side, [string, string]> = {
  web: ["Browser", "#8be9fd"],
  term: ["Dashboard", "#ff79c6"],
  shell: ["Terminal", "#50fa7b"],
  agent: ["Agent (MCP)", "#f1fa8c"],
};
type Shot = { clip: string; side: Side; from: number; to: number; caption: string };
type Story = { shots: Shot[]; starts: Partial<Record<Side, number>>; videos: Partial<Record<Side, string>> };

const now = () => Date.now();
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

// Headless pages draw no pointer: a dot shows where the visitor's mouse is.
const CURSOR = () => {
  addEventListener("DOMContentLoaded", () => {
    const dot = document.createElement("div");
    dot.style.cssText =
      "position:fixed;left:-40px;top:-40px;width:18px;height:18px;margin:-9px 0 0 -9px;border-radius:50%;" +
      "background:rgb(255 121 198 / 75%);border:2px solid #fff;box-shadow:0 1px 6px rgb(0 0 0 / 40%);" +
      "z-index:2147483647;pointer-events:none;transition:transform .1s";
    document.documentElement.append(dot);
    const at = (event: MouseEvent) => {
      dot.style.left = `${event.clientX}px`;
      dot.style.top = `${event.clientY}px`;
    };
    addEventListener("mousemove", at, true);
    addEventListener("mousedown", () => (dot.style.transform = "scale(0.6)"), true);
    addEventListener("mouseup", () => (dot.style.transform = ""), true);
  });
};

const shares = (): { id: string | number; url: string; password?: string }[] =>
  JSON.parse(spawnSync("bunflared", ["ls", "--json"], { encoding: "utf8" }).stdout || "[]");

// Requests from this script go through the local end of the share: through
// Cloudflare, their headers would carry this computer's public address.
function localEnd(pid: string) {
  const listening = spawnSync("lsof", ["-nP", "-a", "-p", pid, "-iTCP", "-sTCP:LISTEN", "-Fn"], { encoding: "utf8" });
  const port = listening.stdout.match(/:(\d+)\n/)?.[1];
  if (!port) throw new Error("the share's local port was not found");
  return `http://127.0.0.1:${port}`;
}

// One recording: the sides filmed, their start times, and the shots cut out
// of them.
class Take {
  shots: Shot[] = [];
  starts: Partial<Record<Side, number>> = {};
  private contexts = new Map<Side, BrowserContext>();
  private pages = new Map<Side, Page>();
  private servers: ChildProcess[] = [];
  constructor(readonly browser: Browser, readonly work: string) {}

  shot(clip: string, side: Side, from: number, to: number, caption: string) {
    this.shots.push({ clip, side, from, to, caption });
  }

  private async film(side: Side, options: Parameters<Browser["newContext"]>[0] = {}) {
    const context = await this.browser.newContext({ viewport: SIZE, recordVideo: { dir: join(this.work, side), size: SIZE }, ...options });
    const page = await context.newPage();
    this.contexts.set(side, context);
    this.pages.set(side, page);
    this.starts[side] = now();
    return { context, page };
  }

  async terminal(side: Exclude<Side, "web">, argv: string[], env: Record<string, string> = {}) {
    this.servers.push(
      spawn(
        "ttyd",
        [
          "-p", String(PORTS[side]), "-W", "-o",
          "-t", "fontSize=15", "-t", "fontFamily=Menlo", "-t", `theme=${JSON.stringify(DRACULA)}`,
          "-t", "disableLeaveAlert=true", "-t", "disableResizeOverlay=true", "-t", "disableReconnect=true",
          "-t", "enableSixel=true",
          ...argv,
        ],
        { cwd: this.work, stdio: "ignore", env: { ...process.env, ...env } },
      ),
    );
    await sleep(800);
    const { page } = await this.film(side);
    await page.goto(`http://127.0.0.1:${PORTS[side]}`);
    await page.waitForFunction(() => (window as any).term);
    return new Terminal(page);
  }

  async visitor(url: string) {
    const { context, page } = await this.film("web", { userAgent: USER_AGENT });
    await context.addInitScript(CURSOR);
    await sleep(600);
    await page.goto(url);
    return new Visitor(page);
  }

  async close(): Promise<Story> {
    for (const context of this.contexts.values()) await context.close();
    for (const server of this.servers) server.kill();
    const videos: Partial<Record<Side, string>> = {};
    for (const [side, page] of this.pages) videos[side] = (await page.video()!.path()) as string;
    return { shots: this.shots, starts: this.starts, videos };
  }
}

class Terminal {
  constructor(readonly page: Page) {}
  screen() {
    return this.page.evaluate(() => {
      const buffer = (window as any).term.buffer.active;
      const lines: string[] = [];
      for (let i = 0; i < buffer.length; i++) lines.push(buffer.getLine(i)?.translateToString(true) ?? "");
      return lines.join("\n");
    });
  }
  async wait(pattern: RegExp, timeout = 60_000) {
    const until = now() + timeout;
    while (now() < until) {
      const text = await this.screen();
      if (pattern.test(text)) return { text, at: now() };
      await sleep(100);
    }
    throw new Error(`the terminal never showed ${pattern}`);
  }
  async press(key: string) {
    await this.page.locator(".xterm-helper-textarea").focus();
    await this.page.keyboard.press(key);
  }
  async type(text: string) {
    await this.page.locator(".xterm-helper-textarea").focus();
    await this.page.keyboard.type(text, { delay: TYPE_MS });
  }
  paste(text: string) {
    return this.page.evaluate((text) => (window as any).term.paste(text), text);
  }
}

class Visitor {
  mouse = { x: SIZE.width / 2, y: SIZE.height / 2 };
  constructor(readonly page: Page) {}
  widget(selector: string) {
    return this.page.locator(`bunflared-feedback ${selector}`);
  }
  // Eased, at a person's pace, never a jump.
  async glide(x: number, y: number) {
    const from = this.mouse;
    const steps = Math.max(10, Math.round(Math.hypot(x - from.x, y - from.y) / GLIDE_PX));
    for (let i = 1; i <= steps; i++) {
      const t = i / steps;
      const eased = t < 0.5 ? 2 * t * t : 1 - (-2 * t + 2) ** 2 / 2;
      await this.page.mouse.move(from.x + (x - from.x) * eased, from.y + (y - from.y) * eased);
      await sleep(FRAME_MS);
    }
    this.mouse = { x, y };
  }
  async center(target: Locator) {
    const box = await target.boundingBox();
    if (!box) throw new Error("nothing to point at");
    return { x: box.x + box.width / 2, y: box.y + box.height / 2, box };
  }
  async click(target: Locator) {
    const { x, y } = await this.center(target);
    await this.glide(x, y);
    await sleep(250);
    await this.page.mouse.down();
    await sleep(90);
    await this.page.mouse.up();
  }
  async type(text: string) {
    await this.page.keyboard.type(text, { delay: TYPE_MS });
  }
  async drag(from: { x: number; y: number }, to: { x: number; y: number }) {
    await this.glide(from.x, from.y);
    await sleep(300);
    await this.page.mouse.down();
    await sleep(200);
    await this.glide(to.x, to.y);
    await sleep(400);
    await this.page.mouse.up();
  }
  // A few turns of the wheel, so the page scrolls as a hand would.
  async scrollBy(pixels: number) {
    const turns = Math.ceil(Math.abs(pixels) / 100);
    for (let i = 0; i < turns; i++) {
      await this.page.mouse.wheel(0, Math.sign(pixels) * Math.min(100, Math.abs(pixels) - i * 100));
      await sleep(60);
    }
    await sleep(500);
  }
  shown(selector: string) {
    return this.page.waitForFunction(
      (selector) => {
        const element = document.querySelector("bunflared-feedback")?.shadowRoot?.querySelector(selector) as HTMLElement | null;
        return element && !element.hidden;
      },
      selector,
    );
  }
}

// A shell with a short prompt, and a clipboard that stays in the recording's
// folder instead of the computer's.
function sandbox(work: string) {
  const bin = join(work, "bin");
  mkdirSync(bin, { recursive: true });
  writeFileSync(join(bin, "pbcopy"), `#!/bin/sh\ncat > '${join(work, "clipboard")}'\n`);
  writeFileSync(join(bin, "open"), "#!/bin/sh\nexit 0\n");
  for (const tool of ["pbcopy", "open"]) chmodSync(join(bin, tool), 0o755);
  return { PATH: `${bin}:${process.env.PATH}` };
}

async function live(take: Take) {
  const before = new Set(shares().map((share) => String(share.id)));
  const term = await take.terminal("term", ["bunf", String(SHOP), String(API)]);
  let traffic = true;
  try {
    // 1. The link, and a first visitor to talk to.
    const boot = await term.wait(/any key to skip/);
    await sleep(2000);
    take.shot("dashboard", "term", boot.at, boot.at + 2000, "bunf 5174 3000 digs a tunnel to your apps");
    const ready = await term.wait(/any key for the dashboard/, 150_000);
    const share = shares().find((share) => !before.has(String(share.id)));
    if (!share) throw new Error("no new share");
    const local = localEnd(String(share.id));
    // Under LAUNCH, so the key still finds the launch screen.
    await sleep(3400);
    take.shot("dashboard", "term", ready.at - 4500, ready.at, "A dragon sets the burrow on fire, a firefighter bunny hoses it");
    take.shot("dashboard", "term", ready.at, ready.at + 3400, "The link is live, and a carrot calms the dragon down");
    await term.press("Space");

    (async () => {
      for (let i = 1; traffic; i++) {
        fetch(`${local}/_port/${API}/api/items?page=${i}`).catch(() => {});
        if (i % 6 === 0) fetch(`${local}/missing`).catch(() => {});
        await sleep(700 + Math.random() * 900);
      }
    })();

    const arrival = term.wait(/Chrome/);
    const loaded = now() + 600;
    const web = await take.visitor(share.url);
    await web.glide(420, 360);
    await web.shown(".react");
    const arrived = await arrival;
    await sleep(2500);
    take.shot("dashboard", "web", loaded - 300, loaded + 2200, "Your client opens it");
    take.shot("dashboard", "term", arrived.at - 300, arrived.at + 2500, "You see them arrive, with their browser and page");

    // 2. Talking both ways.
    const typing = now();
    await term.press("m");
    await sleep(500);
    await term.type(MESSAGE);
    await sleep(300);
    const sent = now();
    await term.press("Enter");
    await sleep(700);
    await term.press("Escape");
    take.shot("dashboard", "term", typing - 300, sent + 700, "m writes to them");
    await web.shown(".toast");
    await sleep(1800);
    take.shot("dashboard", "web", sent - 200, sent + 2000, "The message waits on their tab");

    const answering = now();
    await web.click(web.widget(".toast"));
    await web.click(web.widget("#answer"));
    await web.type(ANSWER);
    await sleep(250);
    const answered = now();
    await web.page.keyboard.press("Enter");
    await term.wait(/says NaN/);
    await sleep(2800);
    take.shot("dashboard", "web", answering - 200, answered + 700, "They answer from the page");
    take.shot("dashboard", "term", answered - 200, answered + 2800, "Their answer lands in the dashboard");

    const reacting = now();
    await web.click(web.widget(".react button").nth(1));
    const reacted = now();
    await sleep(3000);
    await web.page.keyboard.press("Escape");
    take.shot("dashboard", "web", reacting - 200, reacted + 1400, "They react");
    take.shot("dashboard", "term", reacted - 200, reacted + 2800, "Each reaction has its own show");

    // 3. Driving the demo: follow their pointer, send them to a page.
    const picking = now();
    await term.press("ArrowDown");
    await web.page.waitForFunction(
      () => document.querySelector("bunflared-feedback")?.shadowRoot?.querySelector(".tab.live"),
    );
    await sleep(1200);
    const moving = now();
    for (const [x, y] of [[250, 330], [640, 300], [1010, 360], [1010, 560], [640, 600], [250, 560], [420, 360]]) {
      await web.glide(x, y);
      await sleep(250);
    }
    const moved = now();
    take.shot("drive", "term", picking - 300, picking + 1500, "↑ ↓ picks a visitor");
    take.shot("drive", "web", picking - 100, moved, "Their page says they are followed");
    take.shot("drive", "term", moving, moved, "The radar follows their pointer");

    const going = now();
    await term.press("g");
    await sleep(600);
    await term.type("/checkout");
    await sleep(300);
    const sentAway = now();
    await term.press("Enter");
    await web.page.waitForURL(/checkout/);
    await web.glide(web.mouse.x + 1, web.mouse.y + 1);
    await sleep(2200);
    take.shot("drive", "term", going - 300, sentAway + 500, "g sends them to a page");
    take.shot("drive", "web", sentAway - 300, sentAway + 2200, "Their browser follows");
    await term.press("q");
    await sleep(1500);
  } finally {
    traffic = false;
  }
}

// The inspector, on a local share: a payment fails, the filter finds it, it
// opens, replays, and leaves as a curl that fails the same way in a terminal.
async function inspect(take: Take) {
  const before = new Set(shares().map((share) => String(share.id)));
  const term = await take.terminal("term", ["bunf", String(SHOP), String(API), "--local"], sandbox(take.work));
  const shell = await take.terminal("shell", ["bash", "--noprofile", "--norc", "-i"], {
    PS1: "\\n\\[\\e[32m\\]verdure\\[\\e[0m\\] $ ",
    BASH_SILENCE_DEPRECATION_WARNING: "1",
  });
  let traffic = true;
  try {
    const shown = await term.wait(/any key for the dashboard|requests/, 30_000);
    if (/any key for the dashboard/.test(shown.text)) await term.press("Space");
    const share = shares().find((share) => !before.has(String(share.id)));
    if (!share) throw new Error("no new share");
    const local = localEnd(String(share.id));
    (async () => {
      for (let i = 0; traffic; i++) {
        fetch(`${local}/_port/${API}/api/${i % 3 ? "basket" : "slots"}`).catch(() => {});
        await sleep(600 + Math.random() * 700);
      }
    })();

    const web = await take.visitor(`${share.url}/checkout`);
    await web.page.waitForLoadState("networkidle");
    await sleep(1500);
    const flowing = now();
    await sleep(READ_MS + 500);
    take.shot("inspect", "term", flowing, now(), "Every request through the link, as it comes");

    const paying = now();
    const pay = web.page.locator(".pay");
    await web.click(pay);
    await sleep(READ_MS - 500);
    take.shot("inspect", "web", paying - 300, now(), "A visitor presses Pay");
    const failed = await term.wait(/POST.*orders/);
    traffic = false;
    await sleep(READ_MS);
    take.shot("inspect", "term", failed.at - 400, now(), "The order fails with a 500, and the log glitches");

    const filtering = now();
    await term.press("/");
    await sleep(600);
    await term.type("POST");
    await sleep(500);
    await term.press("Enter");
    await sleep(READ_MS);
    take.shot("inspect", "term", filtering - 200, now(), "/ keeps only the requests that match");

    const opening = now();
    await term.press("Tab");
    await sleep(500);
    await term.press("Tab");
    await sleep(500);
    await term.press("ArrowDown");
    await sleep(800);
    await term.press("Enter");
    await sleep(READ_MS + 1500);
    take.shot("inspect", "term", opening - 200, now(), "Enter opens it: headers, JSON body, timing");

    const replaying = now();
    await term.press("p");
    await sleep(READ_MS);
    take.shot("inspect", "term", replaying - 300, now(), "p sends it again to your local server");

    const copying = now();
    await term.press("c");
    await term.wait(/Copied as curl/);
    await sleep(READ_MS - 500);
    take.shot("inspect", "term", copying - 300, now(), "c copies it as a curl command");

    const curl = readFileSync(join(take.work, "clipboard"), "utf8");
    await sleep(400);
    const pasting = now();
    await shell.paste(curl);
    await sleep(1200);
    await shell.type(" -i");
    await sleep(400);
    await shell.press("Enter");
    await shell.wait(/HTTP\/1\.1 500/);
    await sleep(READ_MS + 500);
    take.shot("inspect", "shell", pasting - 600, now(), "Pasted in a terminal, it gets the same 500");
    await term.press("Escape");
    await term.press("q");
    await sleep(1000);
  } finally {
    traffic = false;
  }
}

// The agent: a small real client of `bunflared mcp`, in its own terminal,
// next to the page it watches. Then Edit, whose changes reach it as a note.
async function agent(take: Take) {
  const asked = (tool: string) => writeFileSync(join(take.work, ASK_FILE), tool);
  const term = await take.terminal("agent", ["bun", import.meta.path, "mcp-client"]);
  const opened = await term.wait(/http:\/\/127\.0\.0\.1:\d+/);
  const url = opened.text.match(/http:\/\/127\.0\.0\.1:\d+/)![0];
  const web = await take.visitor(url);
  await web.page.waitForLoadState("networkidle");
  await term.wait(/arrived/);

  // 1. A note on the pay button names its file and line.
  const basket = web.page.locator(".basket-link");
  const going = now();
  await sleep(1200);
  await web.click(basket);
  await web.page.waitForURL(/checkout/);
  await web.glide(980, 300);
  await sleep(READ_MS);
  take.shot("agent", "web", going, now(), "Your app in the browser: the pay button says NaN");

  const noting = now();
  await web.click(web.widget(".tab"));
  await sleep(700);
  await web.type("The pay button says NaN €");
  await sleep(400);
  await web.click(web.widget(".point"));
  await sleep(600);
  await web.click(web.page.locator(".pay"));
  await sleep(1500);
  await web.click(web.widget(".note .send"));
  await sleep(1200);
  take.shot("agent", "web", noting - 200, now(), "You point at it and send a note");
  const noted = await term.wait(/App\.tsx:24/);
  await sleep(READ_MS + 500);
  take.shot("agent", "agent", noted.at - 600, now(), "The agent gets the file and the line: src/App.tsx:24");

  // 2. It looks at the page when it wants.
  const looking = now();
  asked("screenshot");
  await term.wait(/visible text/);
  await sleep(READ_MS + 1000);
  take.shot("agent", "agent", looking, now(), "It takes a screenshot of the page when it needs one");

  // 3. An error on the page reaches it.
  const coding = now();
  await web.page.keyboard.press("Escape");
  await web.click(web.page.locator("#code"));
  await web.type(BAD_CODE);
  await sleep(400);
  await web.click(web.page.locator(".code button"));
  await sleep(READ_MS - 500);
  take.shot("agent", "web", coding - 200, now(), "A promo code that does nothing");
  const raised = await term.wait(/reading 'percent'/);
  await sleep(READ_MS + 500);
  take.shot("agent", "agent", raised.at - 400, now(), "The JavaScript error reaches the agent, with its line");

  // note.png: three points, each with its own comment and source file.
  await web.click(web.page.locator(".mark"));
  await web.page.waitForURL((url) => url.pathname === "/");
  await web.page.waitForLoadState("networkidle");
  await web.click(web.widget(".tab"));
  const points: [string, string][] = [
    [".hero h1", "Three lines on a laptop, keep it to two"],
    [".offer.chosen .price", "Show the price per delivery too"],
    [".hero-actions .button", "Say where it takes me"],
  ];
  for (const [index, [selector, comment]] of points.entries()) {
    await web.click(web.widget(".point"));
    await sleep(400);
    await web.click(web.page.locator(selector).first());
    await sleep(600);
    await web.click(web.widget(".points textarea").nth(index));
    await web.type(comment);
  }
  await web.glide(560, 640);
  await sleep(800);
  const page = join(take.work, "note-page.png");
  await web.page.screenshot({ path: page });
  await web.click(web.widget(".note .send"));
  await term.wait(/Say where it takes me/);
  await compose(take.browser, take.work, page);
  await web.page.keyboard.press("Escape");

  // Edit: The Chef goes first, warmer and roomier, and the agent gets each change.
  await web.page.evaluate(() => scrollTo({ top: 0 }));
  await sleep(600);
  const editing = now();
  await web.scrollBy((await web.page.locator("#boxes").evaluate((node) => node.getBoundingClientRect().top)) - 8);
  await sleep(1000);
  take.shot("edit", "web", editing, now(), "Three offers on your page");

  const picking = now();
  await web.click(web.widget(".tab"));
  await sleep(700);
  await web.click(web.widget(".edit"));
  await sleep(700);
  const chef = web.page.locator(".offer").nth(2);
  await web.click(chef.locator("img"));
  await sleep(900);
  await web.click(web.widget(".parent"));
  await sleep(READ_MS - 500);
  take.shot("edit", "web", picking - 200, now(), "Edit, then a click picks a block, Parent climbs to the card");

  const dragging = now();
  const grip = await web.center(chef.locator("img"));
  const first = await web.center(web.page.locator(".offer").first());
  await web.drag({ x: grip.x, y: grip.y }, { x: first.box.x + 40, y: first.y });
  await sleep(READ_MS);
  take.shot("edit", "web", dragging - 200, now(), "Dragged first among its neighbours");

  const coloring = now();
  await web.click(web.widget("#tab-Colors"));
  await sleep(600);
  const background = web.widget('input[aria-label="Background"]');
  const swatch = await web.center(background);
  await web.glide(swatch.x, swatch.y);
  await sleep(500);
  await background.evaluate((input: HTMLInputElement) => {
    input.value = "#f6dfcb";
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await sleep(READ_MS);
  take.shot("edit", "web", coloring - 200, now(), "Its background changes on the page at once");

  const sizing = now();
  await web.click(web.widget("#tab-Size"));
  await sleep(600);
  const slider = web.widget('input[aria-label="Padding top and bottom"]');
  const track = await web.center(slider);
  const range = await slider.evaluate((input: HTMLInputElement) => [+input.min, +input.max, +input.value]);
  const at = (value: number) => track.box.x + 8 + ((track.box.width - 16) * (value - range[0])) / (range[1] - range[0]);
  await web.drag({ x: at(range[2]), y: track.y }, { x: at(range[2] + 14), y: track.y });
  await sleep(READ_MS);
  take.shot("edit", "web", sizing - 200, now(), "And its size, with a slider");

  const sending = now();
  await web.click(web.widget(".done"));
  await sleep(1200);
  await web.click(web.widget(".points textarea").last());
  await web.type("Our best box should come first");
  await sleep(500);
  await web.click(web.widget(".note .send"));
  await sleep(1200);
  take.shot("edit", "web", sending - 200, now(), "Done lists each change in the note");
  const changed = await term.wait(/best box should come first/);
  await sleep(READ_MS + 1500);
  take.shot("edit", "agent", changed.at - 600, now(), "Your agent gets the exact changes, and the file to make them in");
}

// note.png: the page with its three numbered points, and under it the source
// file each one came with.
async function compose(browser: Browser, work: string, page: string) {
  const feedback = join(work, "bunflared-feedback");
  const newest = readdirSync(feedback)
    .filter((name) => name.endsWith(".md") && !name.startsWith("session_"))
    .sort((a, b) => statSync(join(feedback, b)).mtimeMs - statSync(join(feedback, a)).mtimeMs)[0];
  const sources = [...readFileSync(join(feedback, newest), "utf8").matchAll(/- Source: `([^`]+)`/g)].map((m) => m[1]);
  const card = await browser.newPage({ viewport: { width: SIZE.width, height: SIZE.height + 72 } });
  const image = readFileSync(page).toString("base64");
  await card.setContent(
    `<body style="margin:0;background:#1f2430;font:600 19px system-ui;color:#f8f8f2">` +
      `<img style="display:block" src="data:image/png;base64,${image}">` +
      `<div style="height:72px;display:flex;align-items:center;gap:36px;padding:0 24px">` +
      `<span style="color:#8be9fd">Sources in the note</span>` +
      sources.map((source, i) => `<span><b style="color:#ff79c6">${i + 1}</b> ${source}</span>`).join("") +
      `</div></body>`,
  );
  await card.screenshot({ path: join(import.meta.dir, "note.png") });
  await card.close();
  console.log(join(import.meta.dir, "note.png"));
}

// password.png: the page that asks for the password, on a phone.
async function password(browser: Browser) {
  const started = spawnSync("bunflared", [String(SHOP), "--password", "--detach"], { encoding: "utf8" });
  const share = JSON.parse(started.stdout.trim().split("\n").pop() || "{}");
  if (!share.url) throw new Error(`no share: ${started.stderr}`);
  try {
    const context = await browser.newContext({ ...PHONE });
    const page = await context.newPage();
    await page.goto(share.url);
    await page.waitForLoadState("networkidle");
    await page.locator("input").first().fill(share.password.slice(0, 6));
    await sleep(500);
    await page.screenshot({ path: join(import.meta.dir, "password.png") });
    await context.close();
    console.log(join(import.meta.dir, "password.png"));
  } finally {
    spawnSync("bunflared", ["down", String(share.id)]);
  }
}

async function cut(browser: Browser, work: string, { shots, starts, videos }: Story) {
  const bands = await browser.newPage({ viewport: { width: SIZE.width, height: BAND } });
  for (const clip of new Set(shots.map((s) => s.clip))) {
    const inputs: string[] = [];
    const filters: string[] = [];
    const own = shots.filter((s) => s.clip === clip);
    for (const [i, s] of own.entries()) {
      const band = join(work, `${clip}-${i}.png`);
      const [where, tint] = SIDES[s.side];
      await bands.setContent(
        `<body style="margin:0;height:${BAND}px;display:flex;align-items:center;gap:14px;padding:0 24px;` +
          `background:#1f2430;color:#f8f8f2;font:600 22px system-ui">` +
          `<span style="color:${tint}">${i + 1} · ${where}</span><span>${s.caption}</span></body>`,
      );
      await bands.screenshot({ path: band });
      const from = (s.from - starts[s.side]!) / 1000;
      inputs.push("-ss", from.toFixed(2), "-t", ((s.to - s.from) / 1000).toFixed(2), "-i", videos[s.side]!);
      inputs.push("-i", band);
      const [video, image] = [i * 2, i * 2 + 1];
      filters.push(
        `[${video}:v]fps=${FPS},scale=${SIZE.width}:${SIZE.height},setsar=1,pad=${SIZE.width}:${SIZE.height + BAND}:0:${BAND}[p${i}]`,
        `[p${i}][${image}:v]overlay=0:0,scale=${GIF_WIDTH}:-2:flags=lanczos[s${i}]`,
      );
    }
    filters.push(
      `${own.map((_, i) => `[s${i}]`).join("")}concat=n=${own.length}:v=1:a=0,split[a][b]`,
      `[a]palettegen=stats_mode=diff:max_colors=${COLORS}[palette]`,
      "[b][palette]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
    );
    const raw = join(work, `${clip}.gif`);
    const out = join(import.meta.dir, `${clip}.gif`);
    const steps: [string, string[]][] = [
      ["ffmpeg", ["-y", "-loglevel", "error", ...inputs, "-filter_complex", filters.join(";"), raw]],
      ["gifsicle", ["-O3", `--lossy=${LOSSY}`, raw, "-o", out]],
    ];
    for (const [command, args] of steps) {
      if (spawnSync(command, args, { stdio: "inherit" }).status !== 0) throw new Error(`${command} failed on ${clip}`);
    }
    console.log(out);
  }
}

// The agent's side of the agent recording: it opens a local share of the
// shop, then prints each event, and takes a screenshot when the recording
// leaves an `ask` file.
async function mcpClient() {
  const C = { dim: "\x1b[2m", tool: "\x1b[36m", key: "\x1b[35m", ok: "\x1b[32m", bad: "\x1b[31m", hi: "\x1b[1m", off: "\x1b[0m" };
  const say = (line = "") => process.stdout.write(`${line}\n`);
  const server = spawn("bunflared", ["mcp"], { stdio: ["pipe", "pipe", "ignore"] });
  const pending = new Map<number, (message: any) => void>();
  let buffer = "";
  server.stdout!.on("data", (chunk) => {
    buffer += chunk;
    for (let end = buffer.indexOf("\n"); end >= 0; end = buffer.indexOf("\n")) {
      const message = JSON.parse(buffer.slice(0, end));
      buffer = buffer.slice(end + 1);
      pending.get(message.id)?.(message);
    }
  });
  let lastId = 0;
  const request = (method: string, params: object) =>
    new Promise<any>((resolve) => {
      const id = ++lastId;
      pending.set(id, resolve);
      server.stdin!.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
    });
  const call = async (name: string, args: object = {}) =>
    JSON.parse((await request("tools/call", { name, arguments: args })).result.content[0].text);
  const where = (source: any) =>
    source ? `${source.file}${source.line ? `:${source.line}` : ""}${source.component ? ` in ${source.component}` : ""}` : "";

  await request("initialize", { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "agent", version: "1" } });
  server.stdin!.write(`${JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" })}\n`);
  say(`${C.hi}agent (MCP)${C.off}  ${C.dim}a small client of bunflared mcp, for this demo${C.off}`);
  say();
  say(`${C.tool}▸ open${C.off} ${C.dim}{ ports: [${SHOP}, ${API}] }${C.off}`);
  const share = await call("open", { ports: [SHOP, API] });
  say(`  ${share.url}`);
  say();
  let cursor = 0;
  let waiting = false;
  for (;;) {
    if (existsSync(ASK_FILE)) {
      unlinkSync(ASK_FILE);
      say(`${C.tool}▸ screenshot${C.off}`);
      const seen = await call("screenshot");
      say(`  ${seen.title} · ${seen.page} · ${seen.screen}`);
      spawnSync("chafa", ["-f", "sixels", "-s", "60x18", seen.screenshot], { stdio: "inherit" });
      say(`  ${C.dim}visible text: ${String(seen.text).replace(/\s+/g, " ").slice(0, 70)}…${C.off}`);
      say();
      waiting = false;
    }
    if (!waiting) {
      say(`${C.tool}▸ events${C.off} ${C.dim}{ since: ${cursor}, wait: 30 }${C.off}`);
      waiting = true;
    }
    const found = await call("events", { since: cursor, wait: 1 });
    cursor = found.cursor;
    for (const event of found.events) {
      if (event.kind === "arrived") say(`  ${C.ok}arrived${C.off}  ${event.device ?? ""} on ${event.page ?? "/"}`);
      if (event.kind === "note") {
        say(`  ${C.key}note${C.off}  ${event.device} on ${event.page}${event.text ? `: “${event.text}”` : ""}`);
        for (const [i, point] of (event.points ?? []).entries()) {
          say(`  ${C.hi}${i + 1}${C.off} ${point.selector}${point.text ? ` "${String(point.text).slice(0, 30)}"` : ""}`);
          if (point.source) say(`    ${C.ok}${where(point.source)}${C.off}`);
          for (const line of String(point.comment ?? "").split("; ").filter(Boolean)) say(`    ${line}`);
        }
      }
      if (event.kind === "error") {
        say(`  ${C.bad}error${C.off}  on ${event.page}: ${event.message}`);
        if (event.source) say(`    ${C.ok}at ${event.source}${C.off}`);
      }
      if (event.kind === "message") say(`  ${C.key}message${C.off}  “${event.text}”`);
    }
    if (found.events.length) {
      say();
      waiting = false;
    }
  }
}

const RECORDINGS: Record<string, (take: Take) => Promise<void>> = { live, inspect, agent };
const mode = process.argv[2] ?? "";
if (mode === "mcp-client") {
  await mcpClient();
} else {
  const browser = await chromium.launch();
  try {
    if (mode === "password") {
      await password(browser);
    } else {
      const kept = existsSync(mode) ? mode : undefined;
      if (!kept && !RECORDINGS[mode]) throw new Error(`bun story.ts ${Object.keys(RECORDINGS).join("|")}|password|<folder>`);
      const work = kept ?? mkdtempSync(join(tmpdir(), `bunflared-${mode}-`));
      let story: Story;
      if (kept) story = JSON.parse(readFileSync(join(work, "story.json"), "utf8"));
      else {
        const take = new Take(browser, work);
        try {
          await RECORDINGS[mode](take);
        } finally {
          story = await take.close();
        }
      }
      writeFileSync(join(work, "story.json"), JSON.stringify(story));
      await cut(browser, work, story);
      console.log(`recording kept in ${work}`);
    }
  } finally {
    await browser.close();
  }
}
