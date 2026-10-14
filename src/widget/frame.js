// A page without a mobile viewport is drawn zoomed out on a phone, and a
// pinch zoom moves what the visitor sees: the widget follows the visible
// area, at its normal size, and stays above the on-screen keyboard.
const fit = () => {
  const view = window.visualViewport;
  if (!view) return;
  const [right, bottom] = [view.offsetLeft + view.width, view.offsetTop + view.height];
  host.style.inset = "0 auto auto 0";
  host.style.transformOrigin = "0 0";
  host.style.transform =
    `translate(${right}px, ${bottom}px) scale(${1 / view.scale}) translate(calc(-100% - 16px), calc(-100% - 16px))`;
  host.style.setProperty("--room", `${view.width * view.scale}px`);
  host.style.setProperty("--tall", `${view.height * view.scale}px`);
};
fit();
window.visualViewport?.addEventListener("resize", fit);
window.visualViewport?.addEventListener("scroll", fit);

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
