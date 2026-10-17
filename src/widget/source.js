// Where a pointed element comes from in the app's code, when its development
// build says so: Svelte marks each element, Vue its component, React its fiber.
const SOURCE_LIB = "https://cdn.jsdelivr.net/npm/bippy@0.7.3/dist/source.js";
const SOURCE_WAIT_MS = 3000;
const RAISED_WAIT_MS = 400;

const upward = function* (node) {
  for (; node; node = node.parentElement) yield node;
};
const whole = (n) => (Number.isInteger(n) ? n : undefined);
const found = (file, line, column, component) =>
  file ? { component: component || undefined, file: String(file), line: whole(line), column: whole(column) } : undefined;

// A file the dev server serves, named as in the project.
const servedPath = (url) => url.replace(location.origin, "").replace(/[?#].*$/, "").replace(/^\//, "");
// Vite's source maps name a file without its folder: the URL the page loaded
// it from has the folder, the map has the line.
const withFolder = (name, url) => (name.includes("/") || !url ? name : servedPath(url));

// A frame of the served code, back to the file and line the developer wrote.
const original = async (frame) => {
  const { symbolicateStack, normalizeFileName } = await import(SOURCE_LIB);
  const [back] = await symbolicateStack([frame]);
  return found(withFolder(normalizeFileName(back.fileName ?? ""), frame.fileName), back.lineNumber, back.columnNumber);
};

// Where an error was raised in the app's own code, as `file:line:column`.
const whereRaised = (stack) => {
  const line = String(stack ?? "").split("\n").find((text) => text.includes(location.origin) && !text.includes("/node_modules/"));
  if (!line) return Promise.resolve(undefined);
  const placed = import(SOURCE_LIB).then(async ({ parseStack }) => {
    const [frame] = parseStack(line);
    const place = frame && (await original(frame));
    return place && [place.file, place.line, place.column].filter((part) => part !== undefined).join(":");
  });
  return Promise.race([placed, new Promise((done) => setTimeout(done, RAISED_WAIT_MS))]).catch(() => undefined);
};

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
  const { getSource, normalizeFileName, parseStack } = await import(SOURCE_LIB);
  const owner = fiber._debugOwner;
  const name = owner?.type?.displayName || owner?.type?.name || owner?.name;
  const servedUrl = (file) =>
    [fiber, owner]
      .flatMap((one) => parseStack(String(one?._debugStack?.stack ?? "")))
      .find((frame) => frame.fileName?.replace(/[?#].*$/, "").endsWith(`/${file}`))?.fileName;
  for (const candidate of [fiber, owner]) {
    const source = candidate && (await getSource(candidate).catch(() => null));
    if (!source?.fileName) continue;
    const file = normalizeFileName(source.fileName);
    return found(withFolder(file, servedUrl(file)), source.lineNumber, source.columnNumber, name);
  }
};
// A few seconds at most, and nothing rather than an error: the note goes
// without it.
const locate = (node) =>
  Promise.race([
    Promise.resolve().then(() => svelteSource(node) ?? vueSource(node) ?? reactSource(node)),
    new Promise((done) => setTimeout(done, SOURCE_WAIT_MS)),
  ]).catch(() => undefined);
