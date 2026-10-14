// The tab on the edge. Where it sits is the visitor's choice, kept for the
// visit, and it tucks half away while the page scrolls.
const PLACE_KEY = "bunflared-place";
const TUCK_AFTER_SCROLL_MS = 700;
const [TOP_MIN, TOP_MAX] = [0.12, 0.8];
const tab = $(".tab");
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
  tab.classList.toggle("left", spot.side === "left");
  tab.classList.toggle("right", spot.side !== "left");
  tab.style.top = `${tabTop()}px`;
}
(window.visualViewport ?? window).addEventListener("resize", place);

let tuckTimer;
addEventListener("scroll", () => {
  if (tab.getAttribute("aria-expanded") === "true") return;
  tab.classList.add("tucked");
  clearTimeout(tuckTimer);
  tuckTimer = setTimeout(() => tab.classList.remove("tucked"), TUCK_AFTER_SCROLL_MS);
}, { passive: true, capture: true });

tab.addEventListener("click", () => toggle(form.hidden));
