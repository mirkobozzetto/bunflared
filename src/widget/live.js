// The live channel to the dashboard: chat, reactions, a followed pointer, and
// the go and reload commands.
const RETRY_MIN_MS = 1000;
const RETRY_MAX_MS = 30000;
const POINTER_EVERY_MS = 125;
const watch = $(".watch");
let live = null;
// True once sent: WebSocket.send itself returns nothing.
const say = (message) => {
  if (live?.readyState !== WebSocket.OPEN) return false;
  live.send(JSON.stringify(message));
  return true;
};

// Only while followed, at most a few times a second, and never at rest: the
// last position of a movement is sent once it settles.
let following = false;
let latest = null;
let pending = null;
let sentAt = 0;
const sendPointer = () => {
  pending = null;
  sentAt = Date.now();
  if (following) say({ type: "pointer", ...latest });
};
addEventListener("pointermove", (event) => {
  if (!following) return;
  latest = { x: Math.round(event.clientX), y: Math.round(event.clientY), w: innerWidth, h: innerHeight };
  pending ??= setTimeout(sendPointer, Math.max(0, POINTER_EVERY_MS - (Date.now() - sentAt)));
}, { passive: true, capture: true });
const follow = (on) => {
  following = on;
  watch.hidden = !on;
  marks.live = on;
  signal();
};
// Presence above does not depend on this channel: a page whose policy
// blocks it keeps reporting, it just cannot be driven.
const handlers = {
  chat: ({ text }) => heard(text),
  go: ({ path }) => {
    const target = new URL(path, location.origin);
    if (target.origin === location.origin) location.assign(target);
  },
  reload: () => location.reload(),
  follow: ({ on }) => follow(on),
  react: ({ emoji }) => cheered(emoji),
};
let retry = RETRY_MIN_MS;
const connect = () => {
  let socket;
  try {
    const scheme = location.protocol === "https:" ? "wss" : "ws";
    socket = new WebSocket(`${scheme}://${location.host}${BASE}/live?sid=${sid}`);
  } catch {
    return;
  }
  socket.addEventListener("open", () => {
    live = socket;
    reactRow.hidden = false;
    retry = RETRY_MIN_MS;
  });
  socket.addEventListener("message", (event) => {
    let message;
    try { message = JSON.parse(event.data); } catch { return; }
    if (Object.hasOwn(handlers, message.type)) handlers[message.type](message);
  });
  socket.addEventListener("close", () => {
    live = null;
    reactRow.hidden = true;
    follow(false);
    setTimeout(connect, retry);
    retry = Math.min(retry * 2, RETRY_MAX_MS);
  });
};
