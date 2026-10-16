// Edit: the element picked is moved among its siblings and restyled on the
// page as a preview. Done makes it a point of the note, with its changes;
// Cancel puts it back as it was.
const [bar, editChip, editName] = [$(".bar"), $(".edit"), $(".bar-name")];
const EDIT_STYLES = ["touch-action"];
const EDIT_BLOCKED = [...BLOCKED, "click", "dragstart"];
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

// The page's own clicks wait while the bar is open.
const editGuard = (event) => {
  if (ours(event)) return;
  swallow(event);
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

function startEdit(node) {
  editing = {
    node,
    info: describe(node),
    bare: !node.hasAttribute("style"),
    kept: keepInline(node, EDIT_STYLES),
    unmark: outline(node, `3px solid ${PINK}`),
  };
  node.style.setProperty("touch-action", "none", "important");
  editName.textContent = pointLabel(editing.info);
  bar.hidden = false;
  placeBar();
  for (const type of EDIT_BLOCKED) addEventListener(type, editGuard, true);
  addEventListener("keydown", editKey, true);
  bar.querySelector("button:enabled, input").focus({ preventScroll: true });
}

// The bar closes; the preview stays for Done and goes for Cancel. Nothing of
// the bar itself stays on the element.
function endEdit(keep) {
  const { node, kept, unmark, bare } = editing;
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
  };
  unmark();
  putInline(node, kept.filter(([prop]) => EDIT_STYLES.includes(prop)));
  if (keep) tidy();
  else undo();
  return undo;
}

const editChanges = () => [];

const doneEdit = () => {
  const { node, info } = editing;
  const changes = editChanges();
  endEdit(true);
  const point = addPoint(node, info);
  openPanel(phone ? editChip : point.field);
  if (wantShot) retake();
};
function cancelEdit() {
  endEdit(false);
  openPanel(editChip);
}

editChip.addEventListener("click", () => startPicking(startEdit, editChip));
$(".done").addEventListener("click", doneEdit);
$(".cancel-edit").addEventListener("click", cancelEdit);
