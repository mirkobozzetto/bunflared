// Where a pointed element comes from in the app's code, when its development
// build says so: Svelte marks each element, Vue its component, React its fiber.
const SOURCE_LIB = "https://cdn.jsdelivr.net/npm/bippy@0.7.3/dist/source.js";
const SOURCE_WAIT_MS = 3000;

const upward = function* (node) {
  for (; node; node = node.parentElement) yield node;
};
const whole = (n) => (Number.isInteger(n) ? n : undefined);
const found = (file, line, column, component) =>
  file ? { component: component || undefined, file: String(file), line: whole(line), column: whole(column) } : undefined;

const svelteSource = (node) => {
  for (const element of upward(node)) {
    const loc = element.__svelte_meta?.loc;
    if (loc?.file) return found(loc.file, loc.line, loc.column);
  }
};
// Vue knows the file of a component, never the line.
const vueSource = (node) => {
  for (const element of upward(node)) {
    const type = element.__vueParentComponent?.type;
    if (type?.__file) return found(type.__file, null, null, type.name || type.__name);
  }
};
const fiberOf = (node) => {
  for (const element of upward(node)) {
    const key = Object.keys(element).find((name) => /^__react(Fiber|InternalInstance)\$/.test(name));
    if (key) return element[key];
  }
};
// React 19 keeps no file on the fiber, only the stack where the element was
// made: bippy maps it back through the dev server's source maps. Its hook
// wants to load before React, so the fiber is read off the element here.
const reactSource = async (node) => {
  const fiber = fiberOf(node);
  if (!fiber) return;
  const { getSource, normalizeFileName } = await import(SOURCE_LIB);
  const owner = fiber._debugOwner;
  const name = owner?.type?.displayName || owner?.type?.name || owner?.name;
  for (const candidate of [fiber, owner]) {
    const source = candidate && (await getSource(candidate).catch(() => null));
    if (source?.fileName) return found(normalizeFileName(source.fileName), source.lineNumber, source.columnNumber, name);
  }
};
// A few seconds at most, and nothing rather than an error: the note goes
// without it.
const locate = (node) =>
  Promise.race([
    Promise.resolve().then(() => svelteSource(node) ?? vueSource(node) ?? reactSource(node)),
    new Promise((done) => setTimeout(done, SOURCE_WAIT_MS)),
  ]).catch(() => undefined);
