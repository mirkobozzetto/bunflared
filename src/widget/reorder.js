// Edit moves the element among the visible children of its block: dragged,
// its siblings make room as it goes; or one place at a time. Dragged out of
// its block's edge, it goes into the block under the pointer.
const UNSEEN = ["script", "style", "template", "link", "meta", "noscript"];
// Never a block for another element: the element goes beside them instead.
const LEAVES = ["p", "h1", "h2", "h3", "h4", "h5", "h6", "a", "button", "label", "span", "strong", "em", "b", "i",
  "small", "code", "img", "svg", "picture", "video", "canvas", "iframe", "input", "textarea", "select"];
// A list item stays in a list, a table row in a table.
const KINDS = ["ul", "ol", "menu", "dl", "table", "thead", "tbody", "tfoot", "tr", "select"];
const SHIFT_MS = 160;
const LIFT_PX = 4;
const EDGE_PX = 48;
// After a jump to another block, the pointer must go this far for the next
// one: both blocks change size, and the element would bounce between them.
const SETTLE_PX = 24;
const calm = matchMedia("(prefers-reduced-motion: reduce)");
const seen = (node) => node !== host && !UNSEEN.includes(node.localName) && getComputedStyle(node).display !== "none";

// The node and its visible siblings, in page order.
const blockOf = (node) =>
  node.parentElement ? [...node.parentElement.children].filter((child) => child === node || seen(child)) : [node];
const kidsOf = (parent, node) => [...parent.children].filter((child) => child !== node && seen(child));
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

// To a place in a block, 1 being the first. A dragged node stays under the
// pointer: only the others glide, in the block it leaves and the one it joins.
function moveInto(node, parent, at, dragged = false) {
  const others = kidsOf(parent, node);
  const index = Math.max(0, Math.min(at - 1, others.length));
  const same = parent === node.parentElement;
  if (same && (!others.length || index + 1 === placeOf(node).at)) return;
  const moving = [...new Set([...others, ...blockOf(node)])];
  glide(dragged ? moving.filter((child) => child !== node) : moving, () => {
    if (index < others.length) others[index].before(node);
    else if (others.length) others.at(-1).after(node);
    else parent.append(node);
  });
}
const moveTo = (node, at, dragged = false) => moveInto(node, node.parentElement, at, dragged);

// Back between the same neighbours as before, hidden ones included.
const putBack = ({ node, parent, next }) => {
  if (next && next.parentNode !== parent) return;
  parent.insertBefore(node, next);
};

// The place under the pointer: in a row, siblings are passed at their middle
// from left to right, line by line; in a column, from top to bottom.
const placeIn = (parent, node, x, y) => {
  const look = getComputedStyle(parent);
  const row = (look.display.includes("flex") && !look.flexDirection.startsWith("column"))
    || (look.display.includes("grid") && look.gridTemplateColumns.split(" ").length > 1)
    || getComputedStyle(node).display.startsWith("inline");
  const bounds = parent.getBoundingClientRect();
  y = Math.max(bounds.top + 1, Math.min(y, bounds.bottom - 1));
  const others = kidsOf(parent, node);
  const index = others.findIndex((child) => {
    const box = layoutBox(child);
    return row ? y < box.top || (y <= box.top + box.height && x < box.left + box.width / 2) : y < box.top + box.height / 2;
  });
  return (index < 0 ? others.length : index) + 1;
};

const depthOf = (node) => {
  let depth = 0;
  for (; node.parentElement; node = node.parentElement) depth++;
  return depth;
};
// The block under the pointer at `level`, the depth of the node's block when
// the drag began: a button goes from row to row, a card from column to
// column. Clearly out of its block, else the block one level up. Never body,
// where a framework's root would lose track of the node.
// ponytail: an empty block never takes the node; it goes beside it.
const blockUnder = (node, x, y, level) => {
  const own = node.parentElement;
  const takes = (block) => block === own || (block && ![document.body, document.documentElement].includes(block)
    && !LEAVES.includes(block.localName) && !node.contains(block) && kidsOf(block, node).length > 0
    && (KINDS.includes(own.localName) || KINDS.includes(block.localName) ? block.localName === own.localName : true));
  const line = [];
  const under = document.elementsFromPoint(x, y).find((child) => child !== host && !node.contains(child));
  for (let at = under; at; at = at.parentElement) line.unshift(at);
  if (takes(line[level])) return line[level];
  const box = own.getBoundingClientRect();
  const away = Math.max(box.left - x, x - box.right, box.top - y, y - box.bottom);
  return away > SETTLE_PX && takes(line[level - 1]) ? line[level - 1] : own;
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
  const level = depthOf(node.parentElement);
  let [moving, last, frame, jumped] = [false, null, 0, null];
  const place = ({ clientX: x, clientY: y }) => {
    const settled = !jumped || Math.hypot(x - jumped.x, y - jumped.y) >= SETTLE_PX;
    const block = settled ? blockUnder(node, x, y, level) : node.parentElement;
    if (block !== node.parentElement) jumped = { x, y };
    moveInto(node, block, placeIn(block, node, x, y), true);
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
