// A page without a mobile viewport is drawn zoomed out on a phone, and a
// pinch zoom moves what the visitor sees: the widget covers the visible area,
// at its normal size, and that area ends above the on-screen keyboard.
const fit = () => {
  const view = window.visualViewport;
  if (!view) return;
  const [width, height] = [view.width * view.scale, view.height * view.scale];
  host.style.transformOrigin = "0 0";
  host.style.transform = `translate(${view.offsetLeft}px, ${view.offsetTop}px) scale(${1 / view.scale})`;
  host.style.width = `${width}px`;
  host.style.height = `${height}px`;
  host.style.setProperty("--room", `${width}px`);
  host.style.setProperty("--tall", `${height}px`);
};
fit();
window.visualViewport?.addEventListener("resize", fit);
window.visualViewport?.addEventListener("scroll", fit);

// A pointer's page coordinates, in the widget's own pixels.
const toLocal = (x, y) => {
  const box = host.getBoundingClientRect();
  const ratio = host.clientWidth / box.width;
  return { x: (x - box.left) * ratio, y: (y - box.top) * ratio };
};

// Light or dark like the page under it. A page with no background of its own
// is white, unless it opts into the computer's dark theme with color-scheme.
const scheme = () => {
  for (const element of [document.body, document.documentElement]) {
    const [r, g, b, alpha = 1] = (getComputedStyle(element).backgroundColor.match(/[\d.]+/g) || []).map(Number);
    if (r !== undefined && alpha > 0.5) return 0.2126 * r + 0.7152 * g + 0.0722 * b > 128 ? "light" : "dark";
  }
  const declared = getComputedStyle(document.documentElement).colorScheme;
  const prefersDark = matchMedia("(prefers-color-scheme: dark)").matches;
  return declared.includes("dark") && (prefersDark || !declared.includes("light")) ? "dark" : "light";
};
const paint = () => { host.dataset.scheme = scheme(); };
