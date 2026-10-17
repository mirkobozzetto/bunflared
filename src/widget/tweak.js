// The edit bar's settings, in tabs as in Cursor's design sidebar. Each starts
// from the element's computed value and previews on the page at once, ahead
// of any rule of the app.
const TABS = ["Layout", "Size", "Colors", "Look"];
const listOf = (...values) => values.map((value) => [value, value]);
const SHADOWS = [["none", "none"], ["small", "0 1px 3px rgb(0 0 0 / 20%)"],
  ["medium", "0 4px 12px rgb(0 0 0 / 20%)"], ["large", "0 12px 32px rgb(0 0 0 / 25%)"]];
const CORNERS = ["top-left", "top-right", "bottom-right", "bottom-left"].map((corner) => `border-${corner}-radius`);
// `flex`: only in a flex box; `box`: in a flex or grid box. A setting with no
// `unit` is in px.
const KNOBS = [
  { tab: "Layout", label: "Display", props: ["display"], options: listOf("block", "flex", "grid", "inline-block") },
  { tab: "Layout", label: "Direction", props: ["flex-direction"], options: listOf("row", "column"), flex: true },
  { tab: "Layout", label: "Align items", props: ["align-items"],
    options: listOf("stretch", "flex-start", "center", "flex-end"), box: true },
  { tab: "Layout", label: "Justify content", props: ["justify-content"],
    options: listOf("flex-start", "center", "flex-end", "space-between"), box: true },
  { tab: "Layout", label: "Gap", props: ["row-gap", "column-gap"], min: 0, max: 64, box: true },
  { tab: "Size", label: "Width", props: ["width"], min: 0, max: 800 },
  { tab: "Size", label: "Height", props: ["height"], min: 0, max: 400 },
  { tab: "Size", label: "Padding top and bottom", props: ["padding-top", "padding-bottom"], min: 0, max: 64 },
  { tab: "Size", label: "Padding left and right", props: ["padding-left", "padding-right"], min: 0, max: 64 },
  { tab: "Size", label: "Margin top and bottom", props: ["margin-top", "margin-bottom"], min: 0, max: 128 },
  { tab: "Size", label: "Margin left and right", props: ["margin-left", "margin-right"], min: 0, max: 128 },
  { tab: "Size", label: "Text size", props: ["font-size"], min: 8, max: 96 },
  { tab: "Colors", label: "Text color", props: ["color"], color: true },
  { tab: "Colors", label: "Background", props: ["background-color"], color: true },
  { tab: "Look", label: "Corner radius", props: CORNERS, min: 0, max: 64 },
  { tab: "Look", label: "Opacity", props: ["opacity"], min: 0, max: 1, step: 0.05, unit: "" },
  { tab: "Look", label: "Shadow", props: ["box-shadow"], options: SHADOWS },
];
const KNOB_PROPS = KNOBS.flatMap((knob) => knob.props);
const inPx = (knob) => !knob.color && !knob.options && knob.unit === undefined;
const swatch = document.createElement("canvas").getContext("2d");

// A canvas writes any CSS color, oklch included, as "#rrggbb", or rgba()
// when it is see-through: a fully clear one is "transparent".
const hexOf = (color) => {
  swatch.fillStyle = "#000";
  swatch.fillStyle = color;
  const value = swatch.fillStyle;
  return /,\s*0\)$/.test(value) ? "transparent" : value;
};
// What a color input can show: no transparency.
const opaque = (color) => {
  if (color.startsWith("#")) return color;
  const rgb = color.match(/[\d.]+/g)?.slice(0, 3) ?? ["255", "255", "255"];
  return `#${rgb.map((n) => Math.round(+n).toString(16).padStart(2, "0")).join("")}`;
};
const numOf = (value) => Math.round(parseFloat(value) * 100) / 100 || 0;

const tabList = $(".tabs");
const tabs = TABS.map((name) => {
  const tab = Object.assign(document.createElement("button"), { type: "button", id: `tab-${name}`, textContent: name });
  const pane = Object.assign(document.createElement("div"), { className: "knobs", id: `pane-${name}` });
  tab.setAttribute("role", "tab");
  tab.setAttribute("aria-controls", pane.id);
  pane.setAttribute("role", "tabpanel");
  pane.setAttribute("aria-labelledby", tab.id);
  tab.addEventListener("click", () => showTab(name));
  tabList.append(tab);
  $(".panes").append(pane);
  return { name, tab, pane };
});
let shownTab = "Size";
function showTab(name, focus = false) {
  shownTab = name;
  for (const { name: each, tab, pane } of tabs) {
    tab.setAttribute("aria-selected", String(each === name));
    tab.tabIndex = each === name ? 0 : -1;
    pane.hidden = each !== name;
    if (each === name && focus) tab.focus();
  }
}
// Left and right go from tab to tab; Alt with them moves the element.
tabList.addEventListener("keydown", (event) => {
  const by = { ArrowLeft: -1, ArrowRight: 1 }[event.key];
  if (!by || event.altKey) return;
  event.preventDefault();
  showTab(TABS[(TABS.indexOf(shownTab) + by + TABS.length) % TABS.length], true);
});
showTab(shownTab);

const knobs = KNOBS.map((knob) => {
  const row = document.createElement("div");
  row.className = "knob";
  row.innerHTML = `<span aria-hidden="true"></span><output aria-hidden="true"></output>
    ${knob.options ? "<select></select>" : `<input type="${knob.color ? "color" : "range"}">`}`;
  const [name, out, input] = ["span", "output", "input, select"].map((tag) => row.querySelector(tag));
  name.textContent = knob.label;
  input.setAttribute("aria-label", knob.label);
  if (knob.step) input.step = String(knob.step);
  tabs.find((tab) => tab.name === knob.tab).pane.append(row);
  const state = { ...knob, out, input, before: [], after: null };
  input.addEventListener("input", () => turnKnob(state));
  return state;
});

function showKnob(knob) {
  if (knob.options) return;
  const value = knob.after ?? knob.before[0];
  const text = inPx(knob) ? `${value} px` : String(value);
  knob.out.textContent = text;
  if (!knob.color) knob.input.setAttribute("aria-valuetext", text);
}

// Direction, alignment and gap only mean something in a flex or grid box.
function fitKnobs(node) {
  const display = getComputedStyle(node).display;
  for (const knob of knobs) {
    if (knob.flex || knob.box) knob.input.disabled = !(knob.flex ? display.includes("flex") : /flex|grid/.test(display));
  }
}

// The element's values now, and sliders wide enough for them. An element
// edited before comes back with its first values and its settings.
function loadKnobs(node, saved) {
  const style = getComputedStyle(node);
  knobs.forEach((knob, index) => {
    const read = (prop) => style.getPropertyValue(prop);
    knob.before = saved?.[index].before ?? knob.props.map((prop) =>
      (knob.color ? hexOf(read(prop)) : knob.options ? read(prop) : numOf(read(prop))));
    knob.after = saved?.[index].after ?? null;
    const now = knob.after ?? knob.before[0];
    if (knob.options) {
      const known = knob.options.some(([, value]) => value === knob.before[0]);
      const options = known ? knob.options : [...knob.options, [knob.before[0], knob.before[0]]];
      knob.input.replaceChildren(...options.map(([text, value]) => new Option(text, value)));
      knob.input.value = now;
    } else if (knob.color) {
      knob.input.value = opaque(now);
    } else {
      knob.input.min = String(Math.floor(Math.min(knob.min, ...knob.before)));
      knob.input.max = String(inPx(knob) ? Math.ceil(Math.max(knob.max, ...knob.before.map((value) => value * 2))) : knob.max);
      knob.input.value = String(now);
    }
    showKnob(knob);
  });
  fitKnobs(node);
}
const knobState = () => knobs.map(({ before, after }) => ({ before, after }));

function turnKnob(knob) {
  const raw = knob.input.value;
  const value = inPx(knob) ? `${raw}px` : raw;
  for (const prop of knob.props) editing.node.style.setProperty(prop, value, "important");
  knob.after = knob.color || knob.options ? raw : Number(raw);
  showKnob(knob);
  if (knob.label === "Display") fitKnobs(editing.node);
}

// "Margin top and bottom 8 -> 16 px", for each setting moved off its value.
const knobChanges = () => knobs
  .filter((knob) => knob.after !== null && knob.before.some((value) => value !== knob.after))
  .map((knob) => `${knob.label} ${[...new Set(knob.before)].join(" and ")} -> ${knob.after}${inPx(knob) ? " px" : ""}`);
