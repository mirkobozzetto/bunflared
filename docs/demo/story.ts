// Films the README clips: a visitor in a browser and the dashboard in a
// terminal, recorded at the same time on one share, then cut into stories
// that go back and forth between the two.
// Needs ttyd, ffmpeg and gifsicle. From docs/demo, with `bun apps.ts` running:
//   bun install && bun story.ts     records, then cuts
//   bun story.ts <folder>           cuts again from a kept recording
import { spawn, spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { type Browser, type BrowserContext, chromium, type Locator, type Page } from "playwright";

const SIZE = { width: 1280, height: 760 };
const BAND = 56;
const GIF_WIDTH = 960;
const FPS = 10;
const COLORS = 128;
const LOSSY = 60;
const TTYD_PORT = 7681;
// A plain desktop Chrome: the dashboard names the visitor's browser from it.
const USER_AGENT =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";
const DRACULA = {
  background: "#282a36", foreground: "#f8f8f2", cursor: "#f8f8f2",
  black: "#21222c", red: "#ff5555", green: "#50fa7b", yellow: "#f1fa8c",
  blue: "#bd93f9", magenta: "#ff79c6", cyan: "#8be9fd", white: "#f8f8f2",
};
const MESSAGE = "Hi! The checkout is ready, have a look";
const ANSWER = "Nice! But the pay button is cut";

type Side = "term" | "web";
type Shot = { clip: string; side: Side; from: number; to: number; caption: string };
type Story = { shots: Shot[]; starts: Record<Side, number>; videos: Record<Side, string> };

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

const screen = (term: Page) =>
  term.evaluate(() => {
    const buffer = (window as any).term.buffer.active;
    const lines: string[] = [];
    for (let i = 0; i < buffer.length; i++) lines.push(buffer.getLine(i)?.translateToString(true) ?? "");
    return lines.join("\n");
  });

async function waitScreen(term: Page, pattern: RegExp, timeout = 60_000) {
  const until = now() + timeout;
  while (now() < until) {
    const text = await screen(term);
    if (pattern.test(text)) return { text, at: now() };
    await sleep(100);
  }
  throw new Error(`the terminal never showed ${pattern}`);
}

const shares = (): { id: string | number; url: string }[] =>
  JSON.parse(spawnSync("bunflared", ["ls", "--json"], { encoding: "utf8" }).stdout || "[]");

// Requests from this script go through the local end of the share: through
// Cloudflare, their headers would carry this computer's public address.
function localEnd(pid: string) {
  const listening = spawnSync("lsof", ["-nP", "-a", "-p", pid, "-iTCP", "-sTCP:LISTEN", "-Fn"], { encoding: "utf8" });
  const port = listening.stdout.match(/:(\d+)\n/)?.[1];
  if (!port) throw new Error("the share's local port was not found");
  return `http://127.0.0.1:${port}`;
}

async function record(browser: Browser, work: string): Promise<Story> {
  const shots: Shot[] = [];
  const starts: Record<Side, number> = { term: 0, web: 0 };
  const shot = (clip: string, side: Side, from: number, to: number, caption: string) =>
    shots.push({ clip, side, from, to, caption });

  const before = new Set(shares().map((share) => String(share.id)));
  const ttyd = spawn(
    "ttyd",
    [
      "-p", String(TTYD_PORT), "-W", "-o",
      "-t", "fontSize=15", "-t", "fontFamily=Menlo", "-t", `theme=${JSON.stringify(DRACULA)}`,
      "-t", "disableLeaveAlert=true", "-t", "disableResizeOverlay=true", "-t", "disableReconnect=true",
      "bunf", "5174", "3000",
    ],
    { cwd: work, stdio: "ignore" },
  );
  await sleep(800);

  const termContext = await browser.newContext({ viewport: SIZE, recordVideo: { dir: join(work, "term"), size: SIZE } });
  const term = await termContext.newPage();
  starts.term = now();
  await term.goto(`http://127.0.0.1:${TTYD_PORT}`);
  await term.waitForFunction(() => (window as any).term);
  const press = async (key: string) => {
    await term.locator(".xterm-helper-textarea").focus();
    await term.keyboard.press(key);
  };
  const type = async (text: string) => {
    await term.locator(".xterm-helper-textarea").focus();
    await term.keyboard.type(text, { delay: 45 });
  };

  let web: Page | undefined;
  let webContext: BrowserContext | undefined;
  let mouse = { x: SIZE.width / 2, y: SIZE.height / 2 };
  const glide = async (x: number, y: number, steps = 20) => {
    await web!.mouse.move(x, y, { steps });
    mouse = { x, y };
  };
  const clickOn = async (target: Locator) => {
    const box = await target.boundingBox();
    if (!box) throw new Error("nothing to click");
    await glide(box.x + box.width / 2, box.y + box.height / 2);
    await sleep(200);
    await web!.mouse.down();
    await sleep(90);
    await web!.mouse.up();
  };
  const widget = (selector: string) => web!.locator(`bunflared-feedback ${selector}`);

  let traffic = true;
  try {
    // 1. The link, and a first visitor to talk to.
    const boot = await waitScreen(term, /any key to skip/);
    await sleep(2500);
    shot("dashboard", "term", boot.at, boot.at + 2500, "bunf 5174 3000 digs a tunnel to your apps");
    const live = await waitScreen(term, /any key for the dashboard/, 150_000);
    const share = shares().find((share) => !before.has(String(share.id)));
    if (!share) throw new Error("no new share");
    const local = localEnd(String(share.id));
    await sleep(500);
    shot("dashboard", "term", live.at - 2000, live.at + 500, "The link is live, anyone can open it");
    await press("Space");

    (async () => {
      for (let i = 1; traffic; i++) {
        fetch(`${local}/_port/3000/api/items?page=${i}`).catch(() => {});
        if (i % 6 === 0) fetch(`${local}/missing`).catch(() => {});
        await sleep(700 + Math.random() * 900);
      }
    })();

    webContext = await browser.newContext({
      viewport: SIZE,
      userAgent: USER_AGENT,
      recordVideo: { dir: join(work, "web"), size: SIZE },
    });
    await webContext.addInitScript(CURSOR);
    web = await webContext.newPage();
    starts.web = now();
    await sleep(1000);
    const arrival = waitScreen(term, /Chrome/);
    await web.goto(share.url);
    const loaded = now();
    await glide(420, 360, 30);
    await web.waitForFunction(
      () => !document.querySelector("bunflared-feedback")?.shadowRoot?.querySelector(".reactions")?.hidden,
    );
    const arrived = await arrival;
    await sleep(2500);
    shot("dashboard", "web", loaded - 300, loaded + 2200, "Your client opens it");
    shot("dashboard", "term", arrived.at - 300, arrived.at + 2500, "You see them arrive, with their browser and page");

    // 2. Talking both ways.
    const typing = now();
    await press("m");
    await sleep(500);
    await type(MESSAGE);
    await sleep(300);
    const sent = now();
    await press("Enter");
    await sleep(700);
    await press("Escape");
    shot("dashboard", "term", typing - 300, sent + 700, "m writes to them");
    await web.waitForFunction(
      () => document.querySelector("bunflared-feedback")?.shadowRoot?.querySelector(".thread li"),
    );
    await sleep(1800);
    shot("dashboard", "web", sent - 200, sent + 2000, "The message pops up on their page");

    const answering = now();
    await clickOn(widget(".reply input"));
    await web.keyboard.type(ANSWER, { delay: 45 });
    await sleep(250);
    const answered = now();
    await web.keyboard.press("Enter");
    await waitScreen(term, /pay button/);
    await sleep(2800);
    shot("dashboard", "web", answering - 200, answered + 700, "They answer from the page");
    shot("dashboard", "term", answered - 200, answered + 2800, "Their answer lands in the dashboard");

    const reacting = now();
    await clickOn(widget(".reactions button").nth(1));
    const reacted = now();
    await sleep(3000);
    shot("dashboard", "web", reacting - 200, reacted + 1400, "They react");
    shot("dashboard", "term", reacted - 200, reacted + 2800, "Each reaction has its own show");

    // 3. Driving the demo: follow their pointer, send them to a page.
    const picking = now();
    await press("ArrowDown");
    await web.waitForFunction(
      () => !document.querySelector("bunflared-feedback")?.shadowRoot?.querySelector(".watch")?.hidden,
    );
    await sleep(1200);
    const moving = now();
    for (const [x, y] of [[250, 330], [640, 300], [1010, 360], [1010, 560], [640, 600], [250, 560], [420, 360]]) {
      await glide(x, y, 22);
      await sleep(250);
    }
    const moved = now();
    shot("drive", "term", picking - 300, picking + 1500, "↑ ↓ picks a visitor");
    shot("drive", "web", picking - 100, moved, "Their page says they are followed");
    shot("drive", "term", moving, moved, "The radar follows their pointer");

    const going = now();
    await press("g");
    await sleep(600);
    await type("/checkout");
    await sleep(300);
    const sentAway = now();
    await press("Enter");
    await web.waitForURL(/checkout/);
    await glide(mouse.x + 1, mouse.y + 1, 1);
    await sleep(2200);
    shot("drive", "term", going - 300, sentAway + 500, "g sends them to a page");
    shot("drive", "web", sentAway - 300, sentAway + 2200, "Their browser follows");

    // 4. A note pinned on the element it is about.
    await clickOn(widget(".open"));
    await web.keyboard.type("The pay button is cut", { delay: 30 });
    await clickOn(widget(".point"));
    await clickOn(web.locator(".pay button"));
    await sleep(2500);
    await web.waitForFunction(() => {
      const shadow = document.querySelector("bunflared-feedback")?.shadowRoot;
      const image = shadow?.querySelector("figure img") as HTMLImageElement | null;
      return image?.complete && image.naturalWidth > 0;
    });
    await glide(1100, 520, 15);
    await sleep(600);
    await web.screenshot({ path: join(import.meta.dir, "note.png") });
    await clickOn(widget(".note .send"));
    await sleep(1500);

    // 5. The inspector: the newest request is a failing one.
    traffic = false;
    await sleep(2500);
    const failing = now();
    await fetch(`${local}/_port/3000/boom`);
    await sleep(1800);
    const opening = now();
    await press("Escape");
    await sleep(300);
    await press("Tab");
    await sleep(250);
    await press("Tab");
    await sleep(500);
    await press("ArrowDown");
    await sleep(700);
    const opened = now();
    await press("Enter");
    await sleep(2800);
    const replaying = now();
    await press("p");
    await sleep(2200);
    shot("inspect", "term", failing - 200, failing + 1800, "A failing request makes the log glitch");
    shot("inspect", "term", opening, opened + 2800, "Tab, ↑ ↓ and Enter open it: headers, bodies, timing");
    shot("inspect", "term", replaying - 300, replaying + 2200, "p replays it to your local server");
    await press("Escape");
    await press("q");
    await sleep(1500);
  } finally {
    traffic = false;
    await termContext.close();
    await webContext?.close();
    ttyd.kill();
  }
  const videos = { term: (await term.video()!.path()) as string, web: (await web!.video()!.path()) as string };
  return { shots, starts, videos };
}

async function cut(browser: Browser, work: string, { shots, starts, videos }: Story) {
  const bands = await browser.newPage({ viewport: { width: SIZE.width, height: BAND } });
  for (const clip of new Set(shots.map((s) => s.clip))) {
    const inputs: string[] = [];
    const filters: string[] = [];
    const own = shots.filter((s) => s.clip === clip);
    for (const [i, s] of own.entries()) {
      const band = join(work, `${clip}-${i}.png`);
      const where = s.side === "web" ? "Browser" : "Terminal";
      const tint = s.side === "web" ? "#8be9fd" : "#ff79c6";
      await bands.setContent(
        `<body style="margin:0;height:${BAND}px;display:flex;align-items:center;gap:14px;padding:0 24px;` +
          `background:#1f2430;color:#f8f8f2;font:600 22px system-ui">` +
          `<span style="color:${tint}">${i + 1} · ${where}</span><span>${s.caption}</span></body>`,
      );
      await bands.screenshot({ path: band });
      const from = (s.from - starts[s.side]) / 1000;
      inputs.push("-ss", from.toFixed(2), "-t", ((s.to - s.from) / 1000).toFixed(2), "-i", videos[s.side]);
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

const browser = await chromium.launch();
try {
  const kept = process.argv[2];
  const work = kept ?? mkdtempSync(join(tmpdir(), "bunflared-story-"));
  const story: Story = kept ? JSON.parse(readFileSync(join(work, "story.json"), "utf8")) : await record(browser, work);
  writeFileSync(join(work, "story.json"), JSON.stringify(story));
  await cut(browser, work, story);
  console.log(`recording kept in ${work}`);
} finally {
  await browser.close();
}
