const [chat, thread, reply, answer] = [$(".chat"), $(".thread"), $(".reply"), $(".reply input")];
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
const float = (emoji, big) => {
  const item = document.createElement("span");
  item.className = big ? "float big" : "float";
  item.textContent = emoji;
  item.addEventListener("animationend", () => item.remove());
  root.append(item);
};
reactions.addEventListener("click", (event) => {
  const emoji = event.target.closest("button")?.textContent;
  if (!emoji || !say({ type: "react", emoji, page: page() })) return;
  float(emoji, false);
});
const follow = (on) => {
  following = on;
  watch.hidden = !on;
};
const bubble = (text, mine) => {
  const item = document.createElement("li");
  item.textContent = text;
  if (mine) item.className = "mine";
  thread.append(item);
  while (thread.children.length > THREAD_KEEP) thread.firstElementChild.remove();
  thread.scrollTop = thread.scrollHeight;
};
$(".hide").addEventListener("click", () => { chat.hidden = true; });
reply.addEventListener("submit", (event) => {
  event.preventDefault();
  const text = answer.value.trim();
  if (!text) return;
  if (live?.readyState !== WebSocket.OPEN) {
    answer.placeholder = "Not connected, try again in a moment.";
    return;
  }
  say({ type: "chat", text, page: page() });
  bubble(text, true);
  answer.value = "";
});

// Presence above does not depend on this channel: a page whose policy
// blocks it keeps reporting, it just cannot be driven.
const handlers = {
  chat: ({ text }) => {
    bubble(text, false);
    paint();
    chat.hidden = false;
  },
  go: ({ path }) => {
    const target = new URL(path, location.origin);
    if (target.origin === location.origin) location.assign(target);
  },
  reload: () => location.reload(),
  follow: ({ on }) => follow(on),
  react: ({ emoji }) => float(emoji, true),
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
    reactions.hidden = false;
    retry = RETRY_MIN_MS;
  });
  socket.addEventListener("message", (event) => {
    let message;
    try { message = JSON.parse(event.data); } catch { return; }
    if (Object.hasOwn(handlers, message.type)) handlers[message.type](message);
  });
  socket.addEventListener("close", () => {
    live = null;
    reactions.hidden = true;
    follow(false);
    setTimeout(connect, retry);
    retry = Math.min(retry * 2, RETRY_MAX_MS);
  });
};
