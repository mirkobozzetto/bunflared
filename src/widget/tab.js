// The tab on the edge. Where it sits is the visitor's choice, kept for the
// visit, and it tucks half away while the page scrolls.
const PLACE_KEY = "bunflared-place";
const TUCK_AFTER_SCROLL_MS = 700;
const TOAST_MS = 4000;
const [TOP_MIN, TOP_MAX] = [0.12, 0.8];
const [tab, sliver, badge, toast] = [$(".tab"), $(".sliver"), $(".badge"), $(".toast")];
const liveDot = $(".tab .dot");
const spot = { side: "right", y: phone ? 0.56 : 0.7, hidden: false };
try {
  Object.assign(spot, JSON.parse(sessionStorage.getItem(PLACE_KEY)));
} catch {}
const keep = () => {
  try {
    sessionStorage.setItem(PLACE_KEY, JSON.stringify(spot));
  } catch {}
};
const tabTop = () => Math.round(Math.min(TOP_MAX, Math.max(TOP_MIN, spot.y)) * host.clientHeight);

function place() {
  for (const node of [tab, sliver]) {
    node.classList.toggle("left", spot.side === "left");
    node.classList.toggle("right", spot.side !== "left");
  }
  tab.style.top = `${tabTop()}px`;
  sliver.style.top = `${tabTop() - 13}px`;
  tab.hidden = spot.hidden;
  sliver.hidden = !spot.hidden;
  toast.style.top = `${tabTop() - 6}px`;
  toast.style.left = spot.side === "left" ? "50px" : "";
  toast.style.right = spot.side === "left" ? "" : "50px";
}

// What waits on the tab: unread messages, and the developer following along.
const marks = { unread: 0, live: false };
function signal() {
  const waiting = marks.unread ? `, ${marks.unread} unread message${marks.unread > 1 ? "s" : ""}` : "";
  tab.setAttribute("aria-label", `Talk to the developer${waiting}${marks.live ? ", live" : ""}`);
  sliver.setAttribute("aria-label", `Show the feedback tab${waiting}`);
  badge.textContent = marks.unread;
  badge.hidden = !marks.unread;
  sliver.classList.toggle("unread", marks.unread > 0);
  tab.classList.toggle("live", marks.live);
  liveDot.hidden = !marks.live;
}

let toastTimer;
function preview(text) {
  toast.lastElementChild.textContent = text;
  toast.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { toast.hidden = true; }, TOAST_MS);
}
toast.addEventListener("click", () => openPanel());

// Hidden, the tab becomes a line; a tap on the line brings it back.
function stow(hidden) {
  spot.hidden = hidden;
  toast.hidden = true;
  keep();
  if (hidden) closePanel(false);
  place();
  (hidden ? sliver : tab).focus({ preventScroll: true });
}
sliver.addEventListener("click", () => stow(false));
(window.visualViewport ?? window).addEventListener("resize", place);

let tuckTimer;
addEventListener("scroll", () => {
  if (tab.getAttribute("aria-expanded") === "true") return;
  tab.classList.add("tucked");
  clearTimeout(tuckTimer);
  tuckTimer = setTimeout(() => tab.classList.remove("tucked"), TUCK_AFTER_SCROLL_MS);
}, { passive: true, capture: true });
