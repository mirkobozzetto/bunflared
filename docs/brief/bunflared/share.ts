#!/usr/bin/env bun
// Puts the local app on a public https address, no account needed. The other
// ports go under "/_port/<port>" and "http://localhost:<port>" in responses is
// rewritten to match, so a frontend calling its API on localhost still works.
//   bun scripts/share.ts 5173 3000

// WebSocket upgrades are refused: proxied through fetch they half-open and
// Vite reloads the page in a loop. A manual reload picks up changes.

const ports = Bun.argv.slice(2).map(Number);
if (
  ports.length === 0 ||
  ports.some((port) => !Number.isInteger(port) || port <= 0)
) {
  console.error(
    "usage: share <main-port> [other-port...]   e.g. share 5173 3000",
  );
  process.exit(1);
}
const [main, ...others] = ports;

if (!Bun.which("cloudflared")) {
  console.error("cloudflared is missing: brew install cloudflared");
  process.exit(1);
}
for (const port of ports) {
  const answers = await fetch(`http://localhost:${port}`, {
    signal: AbortSignal.timeout(3000),
  }).then(
    () => true,
    () => false,
  );
  if (!answers) {
    console.error(`Nothing answers on localhost:${port}. Start the app first.`);
    process.exit(1);
  }
}

const TEXT_TYPES = /javascript|json|html|css|text\/plain/;
const LOCAL_URL = new RegExp(
  `https?://(?:localhost|127\\.0\\.0\\.1):(${ports.join("|")})(?!\\d)`,
  "g",
);
const TUNNEL_URL = /https:\/\/[a-z0-9-]+\.trycloudflare\.com/;

const mount = (port: number) => `/_port/${port}`;
const rewrite = (text: string) =>
  text.replace(LOCAL_URL, (_, port) =>
    Number(port) === main ? "" : mount(Number(port)),
  );

function target(url: URL): string {
  for (const port of others) {
    const prefix = mount(port);
    if (url.pathname === prefix || url.pathname.startsWith(`${prefix}/`)) {
      return `http://localhost:${port}${url.pathname.slice(prefix.length) || "/"}${url.search}`;
    }
  }
  return `http://localhost:${main}${url.pathname}${url.search}`;
}

const proxy = Bun.serve({
  port: 0,
  idleTimeout: 255,
  async fetch(request) {
    if (request.headers.get("upgrade"))
      return new Response(null, { status: 404 });
    const headers = new Headers(request.headers);
    headers.delete("host");
    headers.delete("accept-encoding");
    const hasBody = request.method !== "GET" && request.method !== "HEAD";
    let response: Response;
    try {
      response = await fetch(target(new URL(request.url)), {
        method: request.method,
        headers,
        body: hasBody ? await request.arrayBuffer() : undefined,
        redirect: "manual",
      });
    } catch {
      return new Response("The shared app is not answering on this computer.", {
        status: 502,
      });
    }
    // Bun has already decompressed the body.
    const out = new Headers(response.headers);
    for (const name of [
      "content-encoding",
      "content-length",
      "transfer-encoding",
    ])
      out.delete(name);
    const location = out.get("location");
    if (location) out.set("location", rewrite(location));
    const init = {
      status: response.status,
      statusText: response.statusText,
      headers: out,
    };
    if (!TEXT_TYPES.test(out.get("content-type") ?? ""))
      return new Response(response.body, init);
    return new Response(rewrite(await response.text()), init);
  },
});

console.log("Opening a tunnel...");
const tunnel = Bun.spawn(
  [
    "cloudflared",
    "tunnel",
    "--no-autoupdate",
    "--url",
    `http://localhost:${proxy.port}`,
  ],
  {
    stdout: "ignore",
    stderr: "pipe",
  },
);

function stop(code: number): never {
  tunnel.kill();
  proxy.stop(true);
  process.exit(code);
}
process.on("SIGINT", () => stop(0));
process.on("SIGTERM", () => stop(0));

// cloudflared prints its address on stderr; keep draining it afterwards so it never blocks.
const reader = tunnel.stderr.getReader();
const decoder = new TextDecoder();
let log = "";
let publicUrl = "";
while (!publicUrl) {
  const { value, done } = await reader.read();
  if (done) {
    console.error(log || "cloudflared exited without an address.");
    stop(1);
  }
  log += decoder.decode(value);
  publicUrl = log.match(TUNNEL_URL)?.[0] ?? "";
}
void (async () => {
  while (!(await reader.read()).done);
})();
void tunnel.exited.then(() => {
  console.error("The tunnel closed.");
  stop(1);
});

// A new tunnel name takes a few seconds to exist in DNS, and a lookup made
// before that is cached as "not found" for minutes. Ask Cloudflare's DNS over
// HTTPS, not the local resolver, and only print the address once it resolves.
const host = new URL(publicUrl).host;
process.stdout.write(
  "Waiting for the address to exist, usually 10 to 40 seconds ",
);
let resolved = false;
for (let attempt = 0; attempt < 90 && !resolved; attempt++) {
  const answer = await fetch(`https://1.1.1.1/dns-query?name=${host}&type=A`, {
    headers: { accept: "application/dns-json" },
  })
    .then(
      (response) =>
        response.json() as Promise<{ Status?: number; Answer?: unknown[] }>,
    )
    .catch(() => ({}) as { Status?: number; Answer?: unknown[] });
  resolved = answer.Status === 0 && Boolean(answer.Answer?.length);
  if (!resolved) {
    process.stdout.write(".");
    await Bun.sleep(1000);
  }
}
console.log(
  resolved ? " ready." : " still not in DNS; try the address in a minute.",
);

const copied =
  Bun.which("pbcopy") !== null &&
  Bun.spawnSync(["pbcopy"], { stdin: Buffer.from(publicUrl) }).exitCode === 0;
console.log(
  `\nShared at ${publicUrl}${copied ? "  (copied to the clipboard)" : ""}\n`,
);
console.log(`  /${" ".repeat(13)}-> localhost:${main}`);
for (const port of others)
  console.log(`  ${mount(port).padEnd(14)}-> localhost:${port}`);
console.log("\nCtrl-C to stop sharing.");
