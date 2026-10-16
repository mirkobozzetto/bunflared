const SCREENSHOT_LIB = "https://cdn.jsdelivr.net/npm/modern-screenshot@4.7.0/+esm";
const PINK = "#ff79c6";

// Undone, an element that had no style attribute is left without one.
const outline = (node, style) => {
  const saved = [node.style.outline, node.style.outlineOffset];
  const bare = !node.hasAttribute("style");
  node.style.outline = style;
  node.style.outlineOffset = "2px";
  return () => {
    [node.style.outline, node.style.outlineOffset] = saved;
    if (bare && !node.style.length) node.removeAttribute("style");
  };
};
const points = [];

// A point's number, as the page shows it on the outline's corner.
const drawNumber = (context, n, x, y) => {
  context.beginPath();
  context.arc(x, y, 11, 0, 2 * Math.PI);
  context.fillStyle = PINK;
  context.fill();
  context.lineWidth = 2;
  context.strokeStyle = "#fff";
  context.stroke();
  context.fillStyle = "#1f2430";
  context.font = "700 12px system-ui, -apple-system, sans-serif";
  context.textAlign = "center";
  context.textBaseline = "middle";
  context.fillText(String(n), x, y + 0.5);
};

// The whole page without the widget, on a background that never comes out
// transparent. Each point is outlined and numbered over the picture: the page
// itself is left alone while it is taken.
const capture = async () => {
  const { domToCanvas } = await import(SCREENSHOT_LIB);
  const page = document.documentElement;
  const background = getComputedStyle(document.body).backgroundColor;
  const boxes = points.map(({ node }, index) => ({ n: index + 1, shown: node.isConnected, rect: node.getBoundingClientRect() }));
  const [left, top] = [scrollX, scrollY];
  const canvas = await domToCanvas(page, {
    scale: 1,
    width: innerWidth,
    height: Math.max(page.scrollHeight, innerHeight),
    backgroundColor: background === "rgba(0, 0, 0, 0)" ? "#ffffff" : background,
    filter: (node) => node !== host,
  });
  const ratio = canvas.width / innerWidth;
  const context = canvas.getContext("2d");
  context.setTransform(ratio, 0, 0, ratio, 0, 0);
  for (const { n, shown, rect } of boxes) {
    if (!shown) continue;
    const [x, y] = [rect.left + left, rect.top + top];
    context.lineWidth = 3;
    context.strokeStyle = PINK;
    context.strokeRect(x - 3.5, y - 3.5, rect.width + 7, rect.height + 7);
    drawNumber(context, n, Math.max(12, x - 5), Math.max(12, y - 5));
  }
  return new Promise((resolve, reject) => {
    canvas.toBlob((blob) => (blob ? resolve(blob) : reject(new Error("no picture"))), "image/png");
  });
};
