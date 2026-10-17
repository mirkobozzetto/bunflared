// Edit: the element picked is moved among its siblings and restyled on the
// page as a preview. Done makes it a point of the note, with its changes;
// Cancel puts it back as it was. While the bar is open, a click on another
// element, Parent or Child leaves this one, a point if it changed, and edits
// the next.
const [bar, editChip, editName] = [$(".bar"), $(".edit"), $(".bar-name")];
const [earlier, later, placeLine] = [$(".earlier"), $(".later"), $(".place")];
const [upButton, downButton] = [$(".parent"), $(".child")];
const EDIT_STYLES = ["touch-action"];
// Not pointerup: a drag ends on it.
const EDIT_BLOCKED = ["pointerdown", "mousedown", "dblclick", "submit", "click", "dragstart"];
let editing = null;
let dragging = false;
// The elements Parent left, for Child to go back down.
let trail = [];
let fingered = false;

// An element's own inline style for these properties, to put back later.
const keepInline = (node, props) =>
  props.map((prop) => [prop, node.style.getPropertyValue(prop), node.style.getPropertyPriority(prop)]);
const putInline = (node, kept) => {
  for (const [prop, value, priority] of kept) {
    if (value) node.style.setProperty(prop, value, priority);
    else node.style.removeProperty(prop);
  }
};

// The page's own clicks wait while the bar is open. A press on the element
// drags it. Another element is edited next: on the press with a mouse, so
// the same press drags it; on the tap with a finger, whose press may scroll.
const editGuard = (event) => {
  if (ours(event)) return;
  swallow(event);
  const { target, type } = event;
  if (type === "pointerdown") fingered = event.pointerType === "touch";
  const own = editing.node.contains(target);
  const press = type === "pointerdown" && !event.button;
  if (own && press) {
    grabNode(editing.node, event, whileDragged);
  } else if (!own && target !== document.body && target !== document.documentElement
    && (press ? !fingered : type === "click" && fingered)) {
    trail = [];
    switchTo(target, event);
  }
};
// Escape cancels; Alt and an arrow move the element a place.
const STEPS = { ArrowUp: -1, ArrowLeft: -1, ArrowDown: 1, ArrowRight: 1 };
const editKey = (event) => {
  const by = event.altKey && STEPS[event.key];
  if (event.key !== "Escape" && !by) return;
  event.preventDefault();
  event.stopPropagation();
  if (by) step(by);
  else cancelEdit();
};

// Above or below the element, whichever leaves it in view: across the middle
// of the window first, the other side when the page cannot scroll that far.
function placeBar() {
  const box = editing.node.getBoundingClientRect();
  const first = box.top + box.height / 2 > innerHeight / 2;
  for (const top of [first, !first]) {
    bar.classList.toggle("top", top);
    const gap = () => {
      const [box, cover] = [editing.node.getBoundingClientRect(), bar.getBoundingClientRect()];
      return top ? box.top - cover.bottom : cover.top - box.bottom;
    };
    const room = gap();
    if (room < 0) scrollBy(0, top ? room - 12 : 12 - room);
    if (gap() >= 0) return;
  }
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
  if (!dragging) clearHover();
  dragging = !done;
  bar.classList.toggle("faded", !done);
  showPlace(done);
  if (done) placeBar();
}
// Parent stops below body; Child goes back down the way Parent came.
function showFamily() {
  const up = editing.node.parentElement;
  upButton.disabled = !up || up === document.body || up === document.documentElement;
  downButton.disabled = !trail.length || !editing.node.contains(trail.at(-1));
}

function listen(on) {
  const method = on ? "addEventListener" : "removeEventListener";
  for (const type of EDIT_BLOCKED) window[method](type, editGuard, true);
  window[method]("keydown", editKey, true);
  window[method]("pointermove", hover, true);
  bar.hidden = !on;
}

// An element already edited comes back with its first values and settings.
function startEdit(node, event) {
  const old = points.find((point) => point.node === node && point.edit);
  if (old) takePoint(old);
  editing = old ? { ...old.edit.session } : {
    node,
    home: { node, parent: node.parentNode, next: node.nextSibling },
    from: placeOf(node),
    info: describe(node),
    bare: !node.getAttribute("style"),
    kept: keepInline(node, [...EDIT_STYLES, ...KNOB_PROPS]),
  };
  // Adjusted, Cancel brings the point back as it was.
  editing.resume = old && { point: old, style: node.getAttribute("style"),
    at: { node, parent: node.parentNode, next: node.nextSibling } };
  editing.unmark = outline(node, `3px solid ${PINK}`);
  node.style.setProperty("touch-action", "none", "important");
  editName.textContent = pointLabel(editing.info);
  loadKnobs(node, old?.edit.session.knobs);
  showPlace(false);
  showFamily();
  const opening = bar.hidden;
  if (opening) listen(true);
  placeBar();
  if (opening || root.activeElement?.disabled) bar.querySelector("button:enabled, input").focus({ preventScroll: true });
  if (!opening) announce(`Editing ${pointLabel(editing.info)}`);
  if (event?.type === "pointerdown") grabNode(node, event, whileDragged);
}

// The element keeps its preview, or goes back as it was; nothing of the bar
// stays on it. Returns how to take the preview back later.
function release(keep) {
  const { node, home, kept, unmark, bare } = editing;
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
function closeBar() {
  clearHover();
  listen(false);
  trail = [];
}

// What a point says: each change, before and after.
const editChanges = () => {
  const { at, of } = placeOf(editing.node);
  const moved = at === editing.from.at ? [] : [`Moved from place ${editing.from.at} to place ${at} of ${of}`];
  return [...moved, ...knobChanges()];
};

// The element left becomes a point when it changed, or when Done wants one.
function leave(always) {
  const { node, info } = editing;
  const changes = editChanges();
  const session = { ...editing, knobs: knobState() };
  const undo = release(changes.length > 0);
  if (!changes.length && !always) return null;
  return addPoint(node, info, changes.length ? { changes, undo, session } : null);
}
function switchTo(node, event) {
  clearHover();
  leave(false);
  startEdit(node, event);
}

const doneEdit = () => {
  const point = leave(true);
  closeBar();
  openPanel(phone ? editChip : point.field);
  if (wantShot) retake();
};
function cancelEdit() {
  const { node, resume, unmark } = editing;
  if (resume) {
    editing = null;
    unmark();
    if (resume.style === null) node.removeAttribute("style");
    else node.setAttribute("style", resume.style);
    putBack(resume.at);
    addPoint(node, resume.point.info, resume.point.edit);
  } else {
    release(false);
  }
  closeBar();
  openPanel(editChip);
}
// "Adjust" brings an edited point back into the bar.
function adjustPoint(point) {
  closePanel(false);
  showFlags(true);
  startEdit(point.node);
}

editChip.addEventListener("click", () => startPicking(startEdit, editChip, true));
earlier.addEventListener("click", () => step(-1));
later.addEventListener("click", () => step(1));
upButton.addEventListener("click", () => {
  trail.push(editing.node);
  switchTo(editing.node.parentElement);
});
downButton.addEventListener("click", () => switchTo(trail.pop()));
$(".done").addEventListener("click", doneEdit);
$(".cancel-edit").addEventListener("click", cancelEdit);
