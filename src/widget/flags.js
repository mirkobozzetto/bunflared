// While the note is written, or the next element picked, every point keeps a
// pink outline and its number on the page. The numbers let clicks and scrolls
// through, and nothing stays once the panel closes or the note is sent.
const flagLayer = $(".flags");
let flagsOn = false;
let unflag = [];

function refreshFlags() {
  for (const undo of unflag.reverse()) undo();
  unflag = [];
  flagLayer.replaceChildren();
  if (!flagsOn) return;
  points.forEach(({ node }, index) => {
    unflag.push(outline(node, `3px solid ${PINK}`));
    const flag = document.createElement("b");
    flag.className = "flag";
    flag.textContent = index + 1;
    flagLayer.append(flag);
  });
  placeFlags();
}

// On the outline's top left corner, and only while the element is in view.
function placeFlags() {
  if (!flagsOn) return;
  [...flagLayer.children].forEach((flag, index) => {
    const { node } = points[index];
    const rect = node.getBoundingClientRect();
    flag.hidden = !node.isConnected || rect.bottom < 0 || rect.top > innerHeight || rect.right < 0 || rect.left > innerWidth;
    const at = toLocal(rect.left - 5, rect.top - 5);
    flag.style.left = `${Math.max(12, at.x)}px`;
    flag.style.top = `${Math.max(12, at.y)}px`;
  });
}

const showFlags = (on) => {
  flagsOn = on;
  refreshFlags();
};
addEventListener("scroll", placeFlags, { passive: true, capture: true });
addEventListener("resize", placeFlags);
window.visualViewport?.addEventListener("scroll", placeFlags);
window.visualViewport?.addEventListener("resize", placeFlags);
