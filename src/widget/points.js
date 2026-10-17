// The points of a note: each element pointed at, numbered in the order it was
// picked, with a comment field of its own.
const [pointList, pointChip, pointWord] = [$(".points"), $(".point"), $(".point span")];
const pointLabel = ({ selector, text }) => `${selector}${text ? ` · "${text.slice(0, 40)}"` : ""}`;

function renumber() {
  points.forEach((point, index) => {
    const n = index + 1;
    point.num.innerHTML = `<span class="sr">Point </span>${n}`;
    point.field.setAttribute("aria-label", `Comment on point ${n}`);
    point.cross.setAttribute("aria-label", `Remove point ${n}`);
  });
  pointWord.textContent = points.length ? "Point at another" : "Point at it";
  pointList.hidden = !points.length;
  refreshFlags();
  ready();
}

// Edit hands over the element as it was described before any move, with its
// changes, how to take them back, and its settings for "Adjust".
function addPoint(node, info = describe(node), edit = null) {
  const row = document.createElement("li");
  row.innerHTML = `<p class="pin"><b class="num"></b><span></span>
    ${edit ? `<button class="chip adjust" type="button">Adjust</button>` : ""}
    <button class="icon unpin" type="button">${ICON.close}</button></p>
    ${edit ? `<ul class="changes"></ul>` : ""}
    <textarea class="comment" rows="2" placeholder="${edit ? "Anything else about it?" : "What about it?"}"></textarea>`;
  const [num, name, field, cross] = ["b", "span", "textarea", ".unpin"].map((tag) => row.querySelector(tag));
  const point = { node, info, row, num, field, cross, edit };
  row.querySelector(".adjust")?.addEventListener("click", () => adjustPoint(point));
  for (const change of edit?.changes ?? []) {
    row.querySelector(".changes").append(Object.assign(document.createElement("li"), { textContent: change }));
  }
  name.textContent = pointLabel(info);
  name.title = info.selector;
  point.field.addEventListener("input", ready);
  point.cross.addEventListener("click", () => removePoint(point));
  points.push(point);
  pointList.append(row);
  renumber();
  announce(`Point ${points.length} added: ${pointLabel(info)}`);
  return point;
}

// Taken back into Edit, a point leaves the list and keeps its preview.
function takePoint(point) {
  points.splice(points.indexOf(point), 1);
  point.row.remove();
  renumber();
}

// The focus moves to the cross now in the same place, so several points go in
// a row; with none left, to the button that adds one.
// An edited element goes back once its pink outline is off.
function removePoint(point) {
  const index = points.indexOf(point);
  takePoint(point);
  point.edit?.undo();
  announce(`Point ${index + 1} removed`);
  ((points[index] ?? points[index - 1])?.cross ?? pointChip).focus();
  if (wantShot) retake();
}

function clearPoints() {
  for (const point of points.splice(0)) point.row.remove();
  renumber();
}
