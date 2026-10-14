// bunflared companion, injected into shared pages: tells the dashboard who is
// on which page, adds a feedback button whose notes land in the project, and
// keeps a live channel open so the dashboard can drive the demo.

if (window.__bunflared) return;
window.__bunflared = true;

const BASE = "/_bunflared";
const SCREENSHOT_LIB = "https://cdn.jsdelivr.net/npm/modern-screenshot@4.7.0/+esm";
const PING_EVERY_MS = 5000;
const RETRY_MIN_MS = 1000;
const RETRY_MAX_MS = 30000;
const POINTER_EVERY_MS = 125;
const REACTIONS = ["👍", "🔥", "😍", "😕"];

const sid = sessionStorage.getItem("bunflared-sid") || Math.random().toString(36).slice(2, 10);
sessionStorage.setItem("bunflared-sid", sid);

let lastActive = Date.now();
let clicks = 0;
const touch = () => { lastActive = Date.now(); };
for (const type of ["pointerdown", "pointermove", "keydown", "scroll", "touchstart"]) {
  addEventListener(type, touch, { passive: true, capture: true });
}
addEventListener("click", () => { clicks += 1; }, { capture: true });

const page = () => location.pathname + location.search;
const state = (gone) => JSON.stringify({
  sid,
  page: page(),
  visible: document.visibilityState === "visible",
  idle: Math.round((Date.now() - lastActive) / 1000),
  clicks,
  gone,
});
const ping = () =>
  fetch(`${BASE}/ping`, { method: "POST", body: state(false), keepalive: true }).catch(() => {});
ping();
setInterval(ping, PING_EVERY_MS);
document.addEventListener("visibilitychange", ping);
addEventListener("popstate", ping);
addEventListener("pagehide", () => navigator.sendBeacon(`${BASE}/ping`, state(true)));
for (const name of ["pushState", "replaceState"]) {
  const original = history[name];
  history[name] = function (...args) {
    const result = original.apply(this, args);
    setTimeout(ping);
    return result;
  };
}
