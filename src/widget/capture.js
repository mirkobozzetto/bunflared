const SCREENSHOT_LIB = "https://cdn.jsdelivr.net/npm/modern-screenshot@4.7.0/+esm";

// Leaves the widget itself out of the picture, and never lets the page
// background come out transparent.
const outline = (node, style) => {
  const saved = [node.style.outline, node.style.outlineOffset];
  node.style.outline = style;
  node.style.outlineOffset = "2px";
  return () => { [node.style.outline, node.style.outlineOffset] = saved; };
};
let pinned = null;
const capture = async () => {
  const { domToBlob } = await import(SCREENSHOT_LIB);
  const page = document.documentElement;
  const background = getComputedStyle(document.body).backgroundColor;
  const restore = pinned?.node.isConnected ? outline(pinned.node, "3px solid #ff79c6") : () => {};
  try {
    return await domToBlob(page, {
      scale: 1,
      width: innerWidth,
      height: Math.max(page.scrollHeight, innerHeight),
      backgroundColor: background === "rgba(0, 0, 0, 0)" ? "#ffffff" : background,
      filter: (node) => node !== host,
    });
  } finally {
    restore();
  }
};
