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

function addPoint(node) {
  const info = describe(node);
  const row = document.createElement("li");
  row.innerHTML = `<p class="pin"><b class="num"></b><span></span>
    <button class="icon unpin" type="button">${ICON.close}</button></p>
    <textarea class="comment" rows="2" placeholder="What about it?"></textarea>`;
  const [num, name, field, cross] = ["b", "span", "textarea", "button"].map((tag) => row.querySelector(tag));
  const point = { node, info, row, num, field, cross };
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

// The focus moves to the cross now in the same place, so several points go in
// a row; with none left, to the button that adds one.
function removePoint(point) {
  const index = points.indexOf(point);
  points.splice(index, 1);
  point.row.remove();
  renumber();
  announce(`Point ${index + 1} removed`);
  ((points[index] ?? points[index - 1])?.cross ?? pointChip).focus();
  if (wantShot) retake();
}

function clearPoints() {
  for (const point of points.splice(0)) point.row.remove();
  renumber();
}
