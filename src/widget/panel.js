// The panel opens on the note: a sheet over a scrim on a phone, a popover
// next to the tab on a computer.
const [panel, scrim] = [$(".panel"), $(".scrim")];
const isOpen = () => !panel.hidden;

// On a phone the keyboard stays shut until the visitor taps the field.
function openPanel(focus = phone ? $(".close") : textarea) {
  paint();
  panel.hidden = false;
  showFlags(true);
  scrim.hidden = !phone;
  tab.setAttribute("aria-expanded", "true");
  toast.hidden = true;
  marks.unread = 0;
  signal();
  thread.scrollTop = thread.scrollHeight;
  placePanel();
  focus.focus({ preventScroll: true });
  opened();
}

function closePanel(refocus = true) {
  if (!isOpen()) return;
  panel.hidden = scrim.hidden = true;
  showFlags(false);
  tab.setAttribute("aria-expanded", "false");
  if (refocus) (spot.hidden ? sliver : tab).focus({ preventScroll: true });
}

// Centered on the tab, and whole in the window.
function placePanel() {
  if (phone || !isOpen()) return;
  const height = panel.offsetHeight;
  const top = tabTop() + 23 - height / 2;
  panel.style.top = `${Math.max(12, Math.min(top, host.clientHeight - height - 12))}px`;
  panel.style.left = spot.side === "left" ? "52px" : "";
  panel.style.right = spot.side === "left" ? "" : "52px";
}
new ResizeObserver(placePanel).observe(panel);

scrim.addEventListener("click", () => closePanel());
$(".close").addEventListener("click", () => closePanel());
$(".hide-tab").addEventListener("click", () => stow(true));

// Escape closes; on a phone the focus stays inside the sheet.
root.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && isOpen()) {
    event.preventDefault();
    closePanel();
    return;
  }
  if (event.key !== "Tab" || !phone || !isOpen()) return;
  const stops = [...panel.querySelectorAll("button, textarea, input")].filter((node) => !node.disabled && node.offsetParent);
  const [first, last] = [stops[0], stops.at(-1)];
  if (root.activeElement === (event.shiftKey ? first : last)) {
    event.preventDefault();
    (event.shiftKey ? last : first).focus();
  }
});

// The keyboard shrinks the visible area: the note and Send stay in view.
window.visualViewport?.addEventListener("resize", () => {
  if (phone && isOpen() && root.activeElement === textarea) panel.scrollTop = panel.scrollHeight;
});
