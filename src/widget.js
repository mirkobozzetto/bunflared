// bunflared companion, injected into shared pages: tells the dashboard who is
// on which page, adds a feedback button whose notes land in the project, and
// keeps a live channel open so the dashboard can drive the demo.
(() => {
  if (window.__bunflared) return;
  window.__bunflared = true;

  const BASE = "/_bunflared";
  const SCREENSHOT_LIB = "https://cdn.jsdelivr.net/npm/modern-screenshot@4.7.0/+esm";
  const PING_EVERY_MS = 5000;
  const RETRY_MIN_MS = 1000;
  const RETRY_MAX_MS = 30000;
  const THREAD_KEEP = 30;
  const POINTER_EVERY_MS = 125;

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

  // A shadow root keeps the app's CSS and this widget's CSS apart.
  const host = document.createElement("bunflared-feedback");
  const root = host.attachShadow({ mode: "open" });
  const mac = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
  root.innerHTML = `
    <style>
      :host { all: initial; position: fixed; right: 16px; bottom: 16px; z-index: 2147483647;
        font: 14px/1.4 system-ui, -apple-system, sans-serif; color-scheme: light dark;
        display: flex; flex-direction: column; align-items: flex-end; gap: 8px; }
      [hidden] { display: none !important; }
      button { font: inherit; cursor: pointer; border-radius: 999px; border: 0; padding: 8px 14px; }
      .open { background: #1f2430; color: #fff; box-shadow: 0 4px 14px rgb(0 0 0 / 25%); }
      .card { width: min(320px, calc(100vw - 32px)); padding: 14px; border-radius: 14px; box-sizing: border-box;
        background: Canvas; color: CanvasText; box-shadow: 0 8px 30px rgb(0 0 0 / 30%);
        display: grid; gap: 10px; }
      .chat header { display: flex; justify-content: space-between; align-items: center; }
      .chat .hide { padding: 2px 8px; background: transparent; color: inherit; }
      .thread { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; max-height: 40vh; overflow-y: auto; }
      .thread li { justify-self: start; max-width: 85%; padding: 6px 10px; border-radius: 12px;
        overflow-wrap: anywhere; background: color-mix(in srgb, #ff79c6 25%, Canvas); animation: pop 0.25s ease-out; }
      .thread li.mine { justify-self: end; background: color-mix(in srgb, CanvasText 10%, Canvas); }
      .reply { display: flex; gap: 6px; }
      .reply input { flex: 1; min-width: 0; font: inherit; padding: 6px 12px; border-radius: 999px;
        border: 1px solid color-mix(in srgb, CanvasText 25%, transparent); }
      .watch { margin: 0; padding: 4px 12px; border-radius: 999px; font-size: 12px;
        background: #1f2430; color: #fff; box-shadow: 0 4px 14px rgb(0 0 0 / 25%); }
      .watch::before { content: ""; display: inline-block; width: 8px; height: 8px; margin-right: 6px;
        border-radius: 50%; background: #ff5555; animation: blink 1.2s ease-in-out infinite; }
      @keyframes pop { from { transform: scale(0.8); opacity: 0; } }
      @keyframes blink { 50% { opacity: 0.3; } }
      @media (prefers-reduced-motion: reduce) { .thread li, .watch::before { animation: none; } }
      textarea { font: inherit; width: 100%; box-sizing: border-box; border-radius: 8px;
        padding: 8px; border: 1px solid color-mix(in srgb, CanvasText 25%, transparent); resize: vertical; }
      label { display: flex; gap: 6px; align-items: center; font-size: 13px; }
      figure { margin: 0; position: relative; }
      figure img { display: block; width: 100%; max-height: 150px; object-fit: cover; object-position: top;
        border-radius: 8px; border: 1px solid color-mix(in srgb, CanvasText 20%, transparent); cursor: zoom-in; }
      figure .tools { position: absolute; right: 6px; top: 6px; display: flex; gap: 4px; }
      figure .tools button { padding: 2px 8px; font-size: 12px; background: rgb(31 36 48 / 80%); color: #fff; }
      .status { margin: 0; min-height: 1.2em; font-size: 13px; opacity: 0.8; }
      .row { display: flex; justify-content: flex-end; align-items: center; gap: 8px; }
      .hint { margin-right: auto; font-size: 12px; opacity: 0.6; }
      .send, .answer { background: #ff79c6; color: #1f2430; font-weight: 600; }
      .close { background: transparent; color: inherit; }
    </style>
    <p class="watch" hidden>Live: the developer sees your pointer</p>
    <section class="card chat" hidden>
      <header>
        <strong>From the developer</strong>
        <button class="hide" type="button" title="Hide the conversation">✕</button>
      </header>
      <ol class="thread" aria-live="polite"></ol>
      <form class="reply">
        <input placeholder="Answer..." maxlength="2000" aria-label="Your answer">
        <button class="answer" type="submit">Send</button>
      </form>
    </section>
    <button class="open" type="button">✎ Feedback</button>
    <form class="card note" hidden>
      <strong>What should change?</strong>
      <textarea rows="4" placeholder="One remark at a time. Paste an image to attach it."></textarea>
      <label><input type="checkbox" checked> Attach a screenshot of this page</label>
      <figure hidden>
        <img alt="Image sent with the note, click to enlarge">
        <span class="tools">
          <button class="retake" type="button" title="Take the screenshot again">↻</button>
          <button class="drop" type="button" title="Send without image">✕</button>
        </span>
      </figure>
      <p class="status" aria-live="polite"></p>
      <div class="row">
        <span class="hint">${mac ? "⌘" : "Ctrl"} Enter to send</span>
        <button class="close" type="button">Close</button>
        <button class="send" type="submit">Send</button>
      </div>
    </form>`;
  const $ = (selector) => root.querySelector(selector);
  const [open, form, textarea] = [$(".open"), $(".note"), $("textarea")];
  const checkbox = $(".note input[type=checkbox]");
  const [figure, preview, status, send] = [$("figure"), $("figure img"), $(".status"), $(".note .send")];
  let image = null;

  const show = (blob) => {
    if (preview.src) URL.revokeObjectURL(preview.src);
    image = blob;
    preview.src = blob ? URL.createObjectURL(blob) : "";
    figure.hidden = !blob;
  };

  // Leaves the widget itself out of the picture, and never lets the page
  // background come out transparent.
  const capture = async () => {
    const { domToBlob } = await import(SCREENSHOT_LIB);
    const page = document.documentElement;
    const background = getComputedStyle(document.body).backgroundColor;
    return domToBlob(page, {
      scale: 1,
      width: innerWidth,
      height: Math.max(page.scrollHeight, innerHeight),
      backgroundColor: background === "rgba(0, 0, 0, 0)" ? "#ffffff" : background,
      filter: (node) => node !== host,
    });
  };
  const retake = async () => {
    status.textContent = "Taking a screenshot...";
    try {
      show(await capture());
      status.textContent = "";
    } catch {
      show(null);
      status.textContent = "No screenshot this time. You can paste one.";
    }
  };

  // Light or dark like the page under it. A page with no background of its own
  // is white, unless it opts into the computer's dark theme with color-scheme.
  const scheme = () => {
    for (const element of [document.body, document.documentElement]) {
      const [r, g, b, alpha = 1] = (getComputedStyle(element).backgroundColor.match(/[\d.]+/g) || []).map(Number);
      if (r !== undefined && alpha > 0.5) return 0.2126 * r + 0.7152 * g + 0.0722 * b > 128 ? "light" : "dark";
    }
    const declared = getComputedStyle(document.documentElement).colorScheme;
    const prefersDark = matchMedia("(prefers-color-scheme: dark)").matches;
    return declared.includes("dark") && (prefersDark || !declared.includes("light")) ? "dark" : "light";
  };

  const toggle = (visible) => {
    host.style.colorScheme = scheme();
    form.hidden = !visible;
    open.hidden = visible;
    if (!visible) return;
    textarea.focus();
    if (checkbox.checked && !image) retake();
  };
  open.addEventListener("click", () => toggle(true));
  $(".close").addEventListener("click", () => toggle(false));
  $(".retake").addEventListener("click", () => { checkbox.checked = true; retake(); });
  $(".drop").addEventListener("click", () => { checkbox.checked = false; show(null); });
  checkbox.addEventListener("change", () => (checkbox.checked ? retake() : show(null)));
  preview.addEventListener("click", () => image && window.open(preview.src, "_blank"));

  form.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      form.requestSubmit();
    } else if (event.key === "Escape") {
      toggle(false);
    }
  });

  textarea.addEventListener("paste", (event) => {
    const item = [...event.clipboardData.items].find((entry) => entry.type.startsWith("image/"));
    if (!item) return;
    checkbox.checked = true;
    show(item.getAsFile());
    status.textContent = "Pasted image attached.";
  });

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    const message = textarea.value.trim();
    if (!message || send.disabled) return;
    send.disabled = true;
    status.textContent = "Sending...";
    let shot = null;
    if (checkbox.checked && image) {
      const response = await fetch(`${BASE}/shot`, { method: "POST", body: image }).catch(() => null);
      if (response?.ok) shot = (await response.json()).shot;
    }
    const response = await fetch(`${BASE}/feedback`, {
      method: "POST",
      body: JSON.stringify({ sid, page: page(), message, screen: `${innerWidth}x${innerHeight}`, shot }),
    }).catch(() => null);
    send.disabled = false;
    if (!response?.ok) {
      status.textContent = "Could not send it. Try again?";
      return;
    }
    textarea.value = "";
    textarea.focus();
    status.textContent = shot ? "Sent with its image. Anything else?" : "Sent. Anything else?";
    // The next note deserves a fresh picture of the page.
    if (checkbox.checked) {
      show(null);
      retake().then(() => {
        status.textContent = shot ? "Sent with its image. Anything else?" : "Sent. Anything else?";
      });
    }
  });

  const [chat, thread, reply, answer] = [$(".chat"), $(".thread"), $(".reply"), $(".reply input")];
  const watch = $(".watch");
  let live = null;
  const say = (message) => live?.readyState === WebSocket.OPEN && live.send(JSON.stringify(message));

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
      host.style.colorScheme = scheme();
      chat.hidden = false;
    },
    go: ({ path }) => {
      const target = new URL(path, location.origin);
      if (target.origin === location.origin) location.assign(target);
    },
    reload: () => location.reload(),
    follow: ({ on }) => follow(on),
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
      retry = RETRY_MIN_MS;
    });
    socket.addEventListener("message", (event) => {
      let message;
      try { message = JSON.parse(event.data); } catch { return; }
      if (Object.hasOwn(handlers, message.type)) handlers[message.type](message);
    });
    socket.addEventListener("close", () => {
      live = null;
      follow(false);
      setTimeout(connect, retry);
      retry = Math.min(retry * 2, RETRY_MAX_MS);
    });
  };
  connect();

  document.body.append(host);
})();
