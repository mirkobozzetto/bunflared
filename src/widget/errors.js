// The page's errors, for the agent: thrown and never caught, promises
// rejected with nobody to handle them, and console.error. The same error
// over and over goes once in a while, with how many times it came.
const ERRORS_EVERY_MS = 500;
const ERROR_AGAIN_MS = 5000;
const ERRORS_PER_FLUSH = 10;
const ERRORS_HELD = 50;
const ERROR_TEXT = 2000;
const heldErrors = new Map();
const errorSentAt = new Map();

const asText = (value) => {
  if (value instanceof Error) return `${value.name}: ${value.message}`;
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value) ?? String(value);
  } catch {
    return String(value);
  }
};
// A file of the app as the dev server serves it, without its origin and cache key.
const ownFile = (url) => url.replace(location.origin, "").replace(/\?t=\d+/, "");

const noteError = (kind, message, source = "", stack = "") => {
  const key = `${kind}|${message}|${source}`;
  const held = heldErrors.get(key);
  if (held) held.count += 1;
  else if (heldErrors.size < ERRORS_HELD) {
    const text = (value) => String(value).slice(0, ERROR_TEXT);
    heldErrors.set(key, { type: "error", kind, message: text(message), source, stack: text(stack), page: page(), count: 1 });
  }
};

const flushErrors = () => {
  if (!agreed) return;
  let sent = 0;
  const now = Date.now();
  for (const [key, error] of heldErrors) {
    if (sent === ERRORS_PER_FLUSH) break;
    if (now - (errorSentAt.get(key) ?? 0) < ERROR_AGAIN_MS) continue;
    if (!say(error)) break;
    heldErrors.delete(key);
    errorSentAt.set(key, now);
    sent += 1;
  }
};
setInterval(flushErrors, ERRORS_EVERY_MS);

addEventListener("error", (event) => {
  if (!(event instanceof ErrorEvent)) return;
  const where = event.filename ? `${ownFile(event.filename)}:${event.lineno}:${event.colno}` : "";
  noteError("error", event.error ? asText(event.error) : event.message, where, event.error?.stack ?? "");
});
addEventListener("unhandledrejection", ({ reason }) => {
  noteError("rejection", asText(reason), "", reason?.stack ?? "");
});
const consoleError = console.error;
console.error = function (...args) {
  consoleError.apply(this, args);
  try {
    noteError("console", args.map(asText).join(" "), "", args.find((arg) => arg instanceof Error)?.stack ?? "");
  } catch {}
};
