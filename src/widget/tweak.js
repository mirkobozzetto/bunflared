// The edit bar's settings. Each starts from the element's computed value and
// previews on the page at once, ahead of any rule of the app.
const KNOBS = [
  { label: "Text color", props: ["color"], color: true },
  { label: "Background", props: ["background-color"], color: true },
  { label: "Text size", props: ["font-size"], min: 8, max: 96 },
  { label: "Margin top and bottom", props: ["margin-top", "margin-bottom"], min: 0, max: 128 },
  { label: "Margin left and right", props: ["margin-left", "margin-right"], min: 0, max: 128 },
];
const KNOB_PROPS = KNOBS.flatMap((knob) => knob.props);
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
const pxOf = (value) => Math.round(parseFloat(value) * 10) / 10 || 0;

const knobs = KNOBS.map((knob) => {
  const row = document.createElement("div");
  row.className = "knob";
  row.innerHTML = `<span aria-hidden="true"></span><output aria-hidden="true"></output>
    <input type="${knob.color ? "color" : "range"}">`;
  const [name, out, input] = ["span", "output", "input"].map((tag) => row.querySelector(tag));
  name.textContent = knob.label;
  input.setAttribute("aria-label", knob.label);
  $(".knobs").append(row);
  const state = { ...knob, out, input, before: [], after: null };
  input.addEventListener("input", () => turnKnob(state));
  return state;
});

function showKnob(knob) {
  const value = knob.after ?? knob.before[0];
  const text = knob.color ? value : `${value} px`;
  knob.out.textContent = text;
  if (!knob.color) knob.input.setAttribute("aria-valuetext", text);
}

// The element's values now, and sliders wide enough for them.
function loadKnobs(node) {
  const style = getComputedStyle(node);
  for (const knob of knobs) {
    const read = (prop) => style.getPropertyValue(prop);
    knob.before = knob.props.map((prop) => (knob.color ? hexOf(read(prop)) : pxOf(read(prop))));
    knob.after = null;
    if (knob.color) {
      knob.input.value = opaque(knob.before[0]);
    } else {
      knob.input.min = String(Math.floor(Math.min(knob.min, ...knob.before)));
      knob.input.max = String(Math.ceil(Math.max(knob.max, ...knob.before.map((value) => value * 2))));
      knob.input.value = String(Math.round(knob.before[0]));
    }
    showKnob(knob);
  }
}

function turnKnob(knob) {
  const value = knob.color ? knob.input.value : `${knob.input.value}px`;
  for (const prop of knob.props) editing.node.style.setProperty(prop, value, "important");
  knob.after = knob.color ? knob.input.value : Number(knob.input.value);
  showKnob(knob);
}

// "Margin top and bottom 8 -> 16 px", for each setting moved off its value.
const knobChanges = () => knobs
  .filter((knob) => knob.after !== null && knob.before.some((value) => value !== knob.after))
  .map((knob) => `${knob.label} ${[...new Set(knob.before)].join(" and ")} -> ${knob.after}${knob.color ? "" : " px"}`);
