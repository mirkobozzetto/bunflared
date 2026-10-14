// "Point at it": the next click on the page picks an element instead of
// acting on it, so the note says exactly what it is about.
const picking = $(".picking");
const selector = (node) => {
  const parts = [];
  for (; node?.nodeType === 1 && node.localName !== "html"; node = node.parentElement) {
    if (node.id) {
      parts.unshift(`#${CSS.escape(node.id)}`);
      break;
    }
    let part = node.localName;
    const named = [...node.classList].find((name) => /^[a-z][\w-]{1,30}$/i.test(name));
    if (named) part += `.${named}`;
    const same = [...(node.parentElement?.children ?? [])].filter((child) => child.localName === node.localName);
    if (same.length > 1) part += `:nth-of-type(${same.indexOf(node) + 1})`;
    parts.unshift(part);
  }
  return parts.join(" > ");
};
const describe = (node) => {
  const rect = node.getBoundingClientRect();
  const text = node.innerText || node.getAttribute("aria-label") || node.getAttribute("alt") || node.value || "";
  return {
    selector: selector(node),
    text: String(text).trim().replace(/\s+/g, " ").slice(0, 120),
    x: Math.round(rect.left + scrollX),
    y: Math.round(rect.top + scrollY),
    w: Math.round(rect.width),
    h: Math.round(rect.height),
  };
};
const ours = (event) => event.composedPath().includes(host);
let unhover = () => {};
let hovered = null;
const hover = (event) => {
  if (ours(event) || event.target === hovered) return;
  unhover();
  hovered = event.target;
  unhover = outline(hovered, `2px dashed ${PINK}`);
};
const swallow = (event) => {
  if (ours(event)) return;
  event.preventDefault();
  event.stopImmediatePropagation();
};
const BLOCKED = ["pointerdown", "mousedown", "pointerup", "mouseup", "dblclick", "submit"];
// The hover outline goes first: the point's own outline replaces it.
const choose = (event) => {
  if (ours(event)) return;
  swallow(event);
  endPicking();
  const point = addPoint(event.target);
  openPanel(phone ? pointChip : point.field);
  if (wantShot) retake();
};
// Escape stops here: it must not close the panel that picking reopens.
const escape = (event) => {
  if (event.key !== "Escape") return;
  event.preventDefault();
  event.stopPropagation();
  stopPicking();
};
const startPicking = () => {
  closePanel(false);
  showFlags(true);
  picking.hidden = false;
  addEventListener("pointermove", hover, true);
  addEventListener("click", choose, true);
  addEventListener("keydown", escape, true);
  for (const type of BLOCKED) addEventListener(type, swallow, true);
  $(".stop").focus({ preventScroll: true });
};
function endPicking() {
  unhover();
  unhover = () => {};
  hovered = null;
  removeEventListener("pointermove", hover, true);
  removeEventListener("click", choose, true);
  removeEventListener("keydown", escape, true);
  for (const type of BLOCKED) removeEventListener(type, swallow, true);
  picking.hidden = true;
}
pointChip.addEventListener("click", startPicking);
const stopPicking = () => {
  endPicking();
  openPanel(pointChip);
};
$(".stop").addEventListener("click", stopPicking);
