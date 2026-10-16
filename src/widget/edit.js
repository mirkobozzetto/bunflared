// Edit: the element picked is moved among its siblings and restyled on the
// page as a preview. Done makes it a point of the note, with its changes;
// Cancel puts it back as it was.
const [bar, editChip, editName] = [$(".bar"), $(".edit"), $(".bar-name")];
const [earlier, later, placeLine] = [$(".earlier"), $(".later"), $(".place")];
const EDIT_STYLES = ["touch-action"];
// Not pointerup: a drag ends on it.
const EDIT_BLOCKED = ["pointerdown", "mousedown", "dblclick", "submit", "click", "dragstart"];
let editing = null;

// An element's own inline style for these properties, to put back later.
const keepInline = (node, props) =>
  props.map((prop) => [prop, node.style.getPropertyValue(prop), node.style.getPropertyPriority(prop)]);
const putInline = (node, kept) => {
  for (const [prop, value, priority] of kept) {
    if (value) node.style.setProperty(prop, value, priority);
    else node.style.removeProperty(prop);
  }
};

// The page's own clicks wait while the bar is open; a press on the element
// drags it.
const editGuard = (event) => {
  if (ours(event)) return;
  swallow(event);
  if (event.type === "pointerdown" && !event.button && editing.node.contains(event.target)) {
    grabNode(editing.node, event, whileDragged);
  }
};
const editKey = (event) => {
  if (event.key !== "Escape") return;
  event.preventDefault();
  event.stopPropagation();
  cancelEdit();
};

// Above or below the element, whichever leaves it in view.
function placeBar() {
  const box = editing.node.getBoundingClientRect();
  const top = box.top + box.height / 2 > innerHeight / 2;
  bar.classList.toggle("top", top);
  const cover = bar.getBoundingClientRect();
  const gap = top ? box.top - cover.bottom : cover.top - box.bottom;
  if (gap < 0) scrollBy(0, top ? gap - 12 : 12 - gap);
}

// Its place in the block, said aloud after a move. A move button that can go
// no further hands the focus to the other one.
function showPlace(say) {
  const { at, of } = placeOf(editing.node);
  placeLine.textContent = `Place ${at} of ${of}`;
  earlier.disabled = at <= 1;
  later.disabled = at >= of;
  if (root.activeElement?.disabled) (earlier.disabled ? later : earlier).focus();
  if (say) announce(`Place ${at} of ${of}`);
}
const step = (by) => {
  moveTo(editing.node, placeOf(editing.node).at + by);
  showPlace(true);
};
// While dragged, the bar fades and lets the pointer through.
function whileDragged(done) {
  bar.classList.toggle("faded", !done);
  showPlace(done);
  if (done) placeBar();
}

function startEdit(node, event) {
  editing = {
    node,
    home: { node, parent: node.parentNode, next: node.nextSibling },
    from: placeOf(node).at,
    info: describe(node),
    bare: !node.getAttribute("style"),
    kept: keepInline(node, [...EDIT_STYLES, ...KNOB_PROPS]),
    unmark: outline(node, `3px solid ${PINK}`),
  };
  node.style.setProperty("touch-action", "none", "important");
  editName.textContent = pointLabel(editing.info);
  loadKnobs(node);
  showPlace(false);
  bar.hidden = false;
  placeBar();
  for (const type of EDIT_BLOCKED) addEventListener(type, editGuard, true);
  addEventListener("keydown", editKey, true);
  bar.querySelector("button:enabled, input").focus({ preventScroll: true });
  if (event?.type === "pointerdown") grabNode(node, event, whileDragged);
}

// The bar closes; the preview stays for Done and goes for Cancel. Nothing of
// the bar itself stays on the element.
function endEdit(keep) {
  const { node, home, kept, unmark, bare } = editing;
  for (const type of EDIT_BLOCKED) removeEventListener(type, editGuard, true);
  removeEventListener("keydown", editKey, true);
  bar.hidden = true;
  editing = null;
  const tidy = () => {
    if (bare && !node.style.length) node.removeAttribute("style");
  };
  const undo = () => {
    putInline(node, kept);
    tidy();
    putBack(home);
  };
  unmark();
  putInline(node, kept.filter(([prop]) => EDIT_STYLES.includes(prop)));
  if (keep) tidy();
  else undo();
  return undo;
}

// What Done sends: each change, before and after.
const editChanges = () => {
  const { at, of } = placeOf(editing.node);
  const moved = at === editing.from ? [] : [`Moved from place ${editing.from} to place ${at} of ${of}`];
  return [...moved, ...knobChanges()];
};

const doneEdit = () => {
  const { node, info } = editing;
  const changes = editChanges();
  const undo = endEdit(true);
  const point = addPoint(node, info, changes.length ? { changes, undo } : null);
  openPanel(phone ? editChip : point.field);
  if (wantShot) retake();
};
function cancelEdit() {
  endEdit(false);
  openPanel(editChip);
}

editChip.addEventListener("click", () => startPicking(startEdit, editChip, true));
earlier.addEventListener("click", () => step(-1));
later.addEventListener("click", () => step(1));
$(".done").addEventListener("click", doneEdit);
$(".cancel-edit").addEventListener("click", cancelEdit);
