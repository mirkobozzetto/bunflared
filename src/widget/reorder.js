// Edit moves the element among the visible children of its block, never out
// of it: dragged, its siblings make room as it goes; or one place at a time.
const UNSEEN = ["script", "style", "template", "link", "meta", "noscript"];
const SHIFT_MS = 160;
const LIFT_PX = 4;
const EDGE_PX = 48;
const calm = matchMedia("(prefers-reduced-motion: reduce)");
const seen = (node) => node !== host && !UNSEEN.includes(node.localName) && getComputedStyle(node).display !== "none";

// The node and its visible siblings, in page order.
const blockOf = (node) =>
  node.parentElement ? [...node.parentElement.children].filter((child) => child === node || seen(child)) : [node];
const placeOf = (node) => {
  const all = blockOf(node);
  return { at: all.indexOf(node) + 1, of: all.length };
};

// Where the layout puts a node, in window pixels, without any transform: a
// sibling still gliding must not make the place flicker.
const layoutBox = (node) => {
  if (node.offsetLeft === undefined) return node.getBoundingClientRect();
  const parent = node.offsetParent;
  let [left, top] = [node.offsetLeft - scrollX, node.offsetTop - scrollY];
  if (parent && parent !== document.body) {
    const box = parent.getBoundingClientRect();
    left = node.offsetLeft + box.left + parent.clientLeft - parent.scrollLeft;
    top = node.offsetTop + box.top + parent.clientTop - parent.scrollTop;
  }
  return { left, top, width: node.offsetWidth, height: node.offsetHeight };
};

// Each node glides from where it was to where the change puts it.
const glide = (nodes, change) => {
  const from = nodes.map((node) => node.getBoundingClientRect());
  change();
  if (calm.matches) return;
  nodes.forEach((node, index) => {
    const to = node.getBoundingClientRect();
    const [x, y] = [from[index].left - to.left, from[index].top - to.top];
    if (!x && !y) return;
    node.animate([{ transform: `translate(${x}px, ${y}px)` }, { transform: "translate(0, 0)" }],
      { duration: SHIFT_MS, easing: "cubic-bezier(.16, 1, .3, 1)", composite: "add" });
  });
};

// To a place in its block, 1 being the first. A dragged node stays under the
// pointer: only the others glide.
function moveTo(node, at, dragged = false) {
  const others = blockOf(node).filter((child) => child !== node);
  const index = Math.max(0, Math.min(at - 1, others.length));
  if (!others.length || index + 1 === placeOf(node).at) return;
  glide(dragged ? others : [...others, node], () => {
    if (index < others.length) others[index].before(node);
    else others.at(-1).after(node);
  });
}

// Back between the same neighbours as before, hidden ones included.
const putBack = ({ node, parent, next }) => {
  if (next && next.parentNode !== parent) return;
  parent.insertBefore(node, next);
};

// The place under the pointer: in a row, siblings are passed at their middle
// from left to right, line by line; in a column, from top to bottom.
const placeUnder = (node, x, y) => {
  const parent = node.parentElement;
  const look = getComputedStyle(parent);
  const row = (look.display.includes("flex") && !look.flexDirection.startsWith("column"))
    || (look.display.includes("grid") && look.gridTemplateColumns.split(" ").length > 1)
    || getComputedStyle(node).display.startsWith("inline");
  const bounds = parent.getBoundingClientRect();
  y = Math.max(bounds.top + 1, Math.min(y, bounds.bottom - 1));
  const others = blockOf(node).filter((child) => child !== node);
  const index = others.findIndex((child) => {
    const box = layoutBox(child);
    return row ? y < box.top || (y <= box.top + box.height && x < box.left + box.width / 2) : y < box.top + box.height / 2;
  });
  return (index < 0 ? others.length : index) + 1;
};

// A press on the element: past a few pixels it is lifted and follows the
// pointer, its siblings making room. The window hears the pointer, since a
// node moved in the page loses its pointer capture. Near the top or bottom
// of the window, the page scrolls, faster closer to the edge. `dragged`
// hears each move, then the drop.
// ponytail: only the window scrolls, not a block that scrolls by itself.
function grabNode(node, event, dragged) {
  const box = layoutBox(node);
  const start = { x: event.clientX, y: event.clientY };
  const hold = { x: event.clientX - box.left, y: event.clientY - box.top };
  const kept = keepInline(node, ["translate"]);
  let [moving, last, frame] = [false, null, 0];
  const place = ({ clientX: x, clientY: y }) => {
    moveTo(node, placeUnder(node, x, y), true);
    const slot = layoutBox(node);
    node.style.setProperty("translate", `${x - hold.x - slot.left}px ${y - hold.y - slot.top}px`, "important");
    dragged(false);
  };
  const scroll = () => {
    frame = 0;
    const near = Math.min(last.clientY, innerHeight - last.clientY);
    if (near >= EDGE_PX) return;
    const before = scrollY;
    scrollBy(0, Math.ceil((EDGE_PX - near) / 3) * (last.clientY < innerHeight / 2 ? -1 : 1));
    if (scrollY === before) return;
    place(last);
    frame = requestAnimationFrame(scroll);
  };
  const follow = (move) => {
    if (move.pointerId !== event.pointerId) return;
    if (!moving && Math.hypot(move.clientX - start.x, move.clientY - start.y) < LIFT_PX) return;
    moving = true;
    last = move;
    place(move);
    if (!frame) frame = requestAnimationFrame(scroll);
  };
  const drop = (up) => {
    if (up.pointerId !== event.pointerId) return;
    removeEventListener("pointermove", follow, true);
    removeEventListener("pointerup", drop, true);
    removeEventListener("pointercancel", drop, true);
    cancelAnimationFrame(frame);
    if (!moving) return;
    glide([node], () => putInline(node, kept));
    dragged(true);
  };
  addEventListener("pointermove", follow, true);
  addEventListener("pointerup", drop, true);
  addEventListener("pointercancel", drop, true);
}
