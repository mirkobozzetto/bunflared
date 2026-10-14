// Dragging the tab: up, down, to the other edge, or against the edge to hide it.
const DRAG_FROM_PX = 6;
const HIDE_EDGE_PX = 10;
let grab = null;
let dragged = false;
tab.addEventListener("pointerdown", (event) => {
  if (event.button) return;
  grab = { x: event.clientX, y: event.clientY, moving: false, edge: false };
  tab.setPointerCapture(event.pointerId);
});
tab.addEventListener("pointermove", (event) => {
  if (!grab) return;
  if (!grab.moving && Math.hypot(event.clientX - grab.x, event.clientY - grab.y) < DRAG_FROM_PX) return;
  grab.moving = true;
  const at = toLocal(event.clientX, event.clientY);
  const width = host.clientWidth;
  grab.edge = at.x <= HIDE_EDGE_PX || at.x >= width - HIDE_EDGE_PX;
  tab.classList.remove("tucked");
  tab.classList.add("dragging");
  tab.classList.toggle("will-hide", grab.edge);
  tab.style.left = `${Math.min(Math.max(at.x, 0), width) - 20}px`;
  tab.style.right = "auto";
  tab.style.top = `${at.y - 23}px`;
});
const drop = (event) => {
  const done = grab;
  grab = null;
  if (!done?.moving) return;
  tab.classList.remove("dragging", "will-hide");
  tab.style.left = tab.style.right = "";
  if (event.type === "pointercancel") return place();
  // The click that ends a drag must not open the panel.
  dragged = true;
  setTimeout(() => { dragged = false; });
  const at = toLocal(event.clientX, event.clientY);
  spot.side = at.x < host.clientWidth / 2 ? "left" : "right";
  spot.y = Math.min(TOP_MAX, Math.max(TOP_MIN, (at.y - 23) / host.clientHeight));
  if (done.edge) return stow(true);
  keep();
  place();
};
tab.addEventListener("pointerup", drop);
tab.addEventListener("pointercancel", drop);
tab.addEventListener("click", () => {
  if (!dragged) isOpen() ? closePanel() : openPanel();
});
