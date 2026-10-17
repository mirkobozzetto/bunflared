// The agent looks at the page: what the visitor sees, or one element of it
// after a reload, sent back with a short account of what is on screen.
const LOOK_KEY = "bunflared-look";
const LOOK_MARGIN = 16;
const LOOK_FIND_MS = 5000;
const LOOK_TEXT = 1000;
const SEEN = "h1, h2, h3, h4, p, li, a, button, label, th, td";

const onScreen = (rect) =>
  rect.width > 0 && rect.height > 0 && rect.bottom > 0 && rect.right > 0 && rect.top < innerHeight && rect.left < innerWidth;

// The texts in view, in page order, each once.
const visibleText = () => {
  const texts = new Set();
  for (const node of document.querySelectorAll(SEEN)) {
    if (!onScreen(node.getBoundingClientRect())) continue;
    const text = (node.innerText || node.getAttribute("aria-label") || "").trim().replace(/\s+/g, " ");
    if (text) texts.add(text.slice(0, 120));
  }
  return [...texts].join(" | ").slice(0, LOOK_TEXT);
};

// An app draws after it loads: the element gets a few seconds to show up.
const findLater = async (wanted) => {
  const until = Date.now() + LOOK_FIND_MS;
  for (;;) {
    const node = document.querySelector(wanted);
    if (node || Date.now() > until) return node;
    await new Promise((done) => setTimeout(done, 100));
  }
};

const around = (node) => {
  const rect = node.getBoundingClientRect();
  const x = Math.max(0, rect.left + scrollX - LOOK_MARGIN);
  const y = Math.max(0, rect.top + scrollY - LOOK_MARGIN);
  return { x, y, w: Math.min(innerWidth - x, rect.width + 2 * LOOK_MARGIN), h: rect.height + 2 * LOOK_MARGIN };
};

const look = async ({ id, selector: wanted }) => {
  const answer = (fields) => say({ type: "shot", id, ...fields });
  try {
    const node = wanted ? await findLater(wanted) : null;
    const blob = await crop(node ? around(node) : { x: 0, y: scrollY, w: innerWidth, h: innerHeight });
    const response = await fetch(`${BASE}/shot`, { method: "POST", body: blob });
    if (!response.ok) throw new Error(`the screenshot was refused (${response.status})`);
    const { shot } = await response.json();
    const screen = `${innerWidth}x${innerHeight}`;
    answer({ shot, title: document.title, page: page(), screen, text: visibleText(), missing: Boolean(wanted && !node) });
  } catch (error) {
    answer({ error: String(error?.message ?? error).slice(0, 300) });
  }
};

// Asked to look after a reload, the page keeps the request across it, and
// answers once its channel is back.
const lookAfterReload = (request) => {
  try {
    sessionStorage.setItem(LOOK_KEY, JSON.stringify(request));
  } catch {}
  location.reload();
};
const lookIfAsked = () => {
  let request = null;
  try {
    request = JSON.parse(sessionStorage.getItem(LOOK_KEY));
    sessionStorage.removeItem(LOOK_KEY);
  } catch {}
  if (request) look(request);
};
