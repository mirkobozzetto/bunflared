// Quick reactions: one tap sends an emoji to the developer, who can send one back.
const reactRow = $(".react");

const float = (emoji, x, y, big) => {
  const item = document.createElement("span");
  item.className = big ? "float big" : "float";
  item.textContent = emoji;
  item.style.left = `${x}px`;
  item.style.top = `${y}px`;
  item.addEventListener("animationend", () => item.remove());
  root.append(item);
};

reactRow.addEventListener("click", (event) => {
  const button = event.target.closest("button");
  const emoji = button?.dataset.emoji;
  if (!emoji || !say({ type: "react", emoji, page: page() })) return;
  const box = button.getBoundingClientRect();
  const at = toLocal(box.left + box.width / 2, box.top);
  float(emoji, at.x - 15, at.y - 10, false);
  announce(`${emoji} sent to the developer`);
});

// The developer's reaction rises next to the tab.
function cheered(emoji) {
  const x = spot.side === "left" ? 52 : host.clientWidth - 108;
  float(emoji, x, tabTop() - 20, true);
}
