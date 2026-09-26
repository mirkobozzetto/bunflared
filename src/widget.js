// bunflared companion, injected into shared pages: tells the dashboard who is
// on which page, and adds a feedback button whose notes land in the project.
(() => {
  if (window.__bunflared) return;
  window.__bunflared = true;

  const BASE = "/_bunflared";
  const SCREENSHOT_LIB = "https://cdn.jsdelivr.net/npm/modern-screenshot@4.7.0/+esm";
  const PING_EVERY_MS = 5000;

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
  root.innerHTML = `
    <style>
      :host { all: initial; position: fixed; right: 16px; bottom: 16px; z-index: 2147483647;
        font: 14px/1.4 system-ui, -apple-system, sans-serif; color-scheme: light dark; }
      [hidden] { display: none !important; }
      button { font: inherit; cursor: pointer; border-radius: 999px; border: 0; padding: 8px 14px; }
      .open { background: #1f2430; color: #fff; box-shadow: 0 4px 14px rgb(0 0 0 / 25%); }
      form { width: min(320px, calc(100vw - 32px)); padding: 14px; border-radius: 14px;
        background: Canvas; color: CanvasText; box-shadow: 0 8px 30px rgb(0 0 0 / 30%);
        display: grid; gap: 10px; }
      textarea { font: inherit; width: 100%; box-sizing: border-box; border-radius: 8px;
        padding: 8px; border: 1px solid color-mix(in srgb, CanvasText 25%, transparent); resize: vertical; }
      label { display: flex; gap: 6px; align-items: center; font-size: 13px; }
      .status { margin: 0; min-height: 1.2em; font-size: 13px; opacity: 0.8; }
      .row { display: flex; justify-content: flex-end; gap: 8px; }
      .send { background: #ff79c6; color: #1f2430; font-weight: 600; }
      .close { background: transparent; color: inherit; }
    </style>
    <button class="open" type="button">✎ Feedback</button>
    <form hidden>
      <strong>What should change?</strong>
      <textarea rows="4" placeholder="One remark at a time. Paste an image to attach it."></textarea>
      <label><input type="checkbox" checked> Attach a screenshot of this page</label>
      <p class="status" aria-live="polite"></p>
      <div class="row">
        <button class="close" type="button">Close</button>
        <button class="send" type="submit">Send</button>
      </div>
    </form>`;
  const [open, form] = [root.querySelector(".open"), root.querySelector("form")];
  const [textarea, checkbox] = [root.querySelector("textarea"), root.querySelector("input")];
  const [status, send] = [root.querySelector(".status"), root.querySelector(".send")];
  let pasted = null;

  const toggle = (show) => {
    form.hidden = !show;
    open.hidden = show;
    if (show) textarea.focus();
  };
  open.addEventListener("click", () => toggle(true));
  root.querySelector(".close").addEventListener("click", () => toggle(false));

  textarea.addEventListener("paste", (event) => {
    const item = [...event.clipboardData.items].find((entry) => entry.type.startsWith("image/"));
    if (!item) return;
    pasted = item.getAsFile();
    status.textContent = "Image attached.";
  });

  const capture = async () => {
    host.style.display = "none";
    try {
      const { domToBlob } = await import(SCREENSHOT_LIB);
      // At least one screen tall, on the page's own background, not transparent.
      const root = document.documentElement;
      const background = getComputedStyle(document.body).backgroundColor;
      return await domToBlob(root, {
        scale: 1,
        width: innerWidth,
        height: Math.max(root.scrollHeight, innerHeight),
        backgroundColor: background === "rgba(0, 0, 0, 0)" ? "#ffffff" : background,
      });
    } finally {
      host.style.display = "";
    }
  };

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    const message = textarea.value.trim();
    if (!message) return;
    send.disabled = true;
    status.textContent = "Sending...";
    let shot = null;
    try {
      const image = pasted || (checkbox.checked ? await capture() : null);
      if (image) {
        const response = await fetch(`${BASE}/shot`, { method: "POST", body: image });
        if (response.ok) shot = (await response.json()).shot;
      }
    } catch {
      // The words matter more than the picture: send them anyway.
    }
    const response = await fetch(`${BASE}/feedback`, {
      method: "POST",
      body: JSON.stringify({ sid, page: page(), message, screen: `${innerWidth}x${innerHeight}`, shot }),
    }).catch(() => null);
    if (response?.ok) {
      textarea.value = "";
      pasted = null;
      status.textContent = shot ? "Sent with a screenshot. Anything else?" : "Sent. Anything else?";
    } else {
      status.textContent = "Could not send it. Try again?";
    }
    send.disabled = false;
  });

  document.body.append(host);
})();
