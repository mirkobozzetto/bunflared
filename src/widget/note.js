// The note: a remark, the elements it is about, and a screenshot of the page.
const [form, textarea, send, status] = [$(".note"), $("#note"), $(".note .send"), $(".status")];
const [shotChip, thumb] = [$(".shot"), $(".thumb")];
let image = null;
let wantShot = true;
let sending = false;

// Without a point the remark is required; with one, any comment will do.
const written = () => Boolean(textarea.value.trim()) || points.some((point) => point.field.value.trim());
const ready = () => { send.disabled = sending || !written(); };

// The line under the note: busy, done or failed.
const tell = (text = "", kind = "") => {
  status.className = `status ${kind}`;
  status.innerHTML = { busy: `<span class="spin" aria-hidden="true"></span>`, ok: ICON.check, error: ICON.alert }[kind] ?? "";
  if (text) status.append(text);
};

const show = (blob) => {
  if (thumb.src) URL.revokeObjectURL(thumb.src);
  image = blob;
  if (blob) thumb.src = URL.createObjectURL(blob);
  else thumb.removeAttribute("src");
  thumb.hidden = !blob;
};

const retake = async () => {
  tell("Taking a screenshot...", "busy");
  try {
    show(await capture());
    tell();
  } catch {
    show(null);
    tell("No screenshot this time. You can paste one.", "error");
  }
};

// The panel opened: a fresh screenshot of the page, unless one is waiting.
function opened() {
  if (wantShot && !image) retake();
}

const want = (on) => {
  wantShot = on;
  shotChip.setAttribute("aria-pressed", String(on));
};
shotChip.addEventListener("click", () => {
  want(!wantShot);
  if (wantShot) return retake();
  show(null);
  tell();
});

textarea.addEventListener("input", ready);
form.addEventListener("keydown", (event) => {
  if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
    event.preventDefault();
    form.requestSubmit();
  }
});

textarea.addEventListener("paste", (event) => {
  const item = [...event.clipboardData.items].find((entry) => entry.type.startsWith("image/"));
  if (!item) return;
  want(true);
  show(item.getAsFile());
  tell("Pasted image attached.", "ok");
});

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!written() || sending) return;
  const message = textarea.value.trim();
  sending = true;
  ready();
  tell("Sending...", "busy");
  let shot = null;
  if (wantShot && image) {
    const response = await fetch(`${BASE}/shot`, { method: "POST", body: image }).catch(() => null);
    if (response?.ok) shot = (await response.json()).shot;
  }
  const response = await fetch(`${BASE}/feedback`, {
    method: "POST",
    body: JSON.stringify({
      sid, page: page(), message, screen: `${innerWidth}x${innerHeight}`, shot,
      points: points.map((point) => ({ ...point.info, comment: point.field.value.trim() })),
    }),
  }).catch(() => null);
  sending = false;
  ready();
  if (!response?.ok) {
    tell("Could not send it. Try again?", "error");
    return;
  }
  textarea.value = "";
  clearPoints();
  if (!phone) textarea.focus();
  const done = shot ? "Sent with its image. Anything else?" : "Sent. Anything else?";
  tell(done, "ok");
  // The next note deserves a fresh picture of the page.
  if (wantShot) {
    show(null);
    retake().then(() => tell(done, "ok"));
  }
});
