const [form, textarea] = [$(".note"), $("textarea")];
const checkbox = $(".note input[type=checkbox]");
const [figure, preview, status, send] = [$("figure"), $("figure img"), $(".status"), $(".note .send")];
const enlarge = $("figure a");
const reactions = $(".reactions");
let image = null;

const show = (blob) => {
  if (preview.src) URL.revokeObjectURL(preview.src);
  image = blob;
  preview.src = blob ? URL.createObjectURL(blob) : "";
  if (blob) enlarge.href = preview.src;
  else enlarge.removeAttribute("href");
  figure.hidden = !blob;
};

const retake = async () => {
  status.textContent = "Taking a screenshot...";
  try {
    show(await capture());
    status.textContent = "";
  } catch {
    show(null);
    status.textContent = "No screenshot this time. You can paste one.";
  }
};

// The panel opened: a fresh screenshot of the page, unless one is waiting.
function opened() {
  if (checkbox.checked && !image) retake();
}
$(".retake").addEventListener("click", () => { checkbox.checked = true; retake(); });
$(".drop").addEventListener("click", () => { checkbox.checked = false; show(null); });
checkbox.addEventListener("change", () => (checkbox.checked ? retake() : show(null)));

form.addEventListener("keydown", (event) => {
  if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
    event.preventDefault();
    form.requestSubmit();
  }
});

textarea.addEventListener("paste", (event) => {
  const item = [...event.clipboardData.items].find((entry) => entry.type.startsWith("image/"));
  if (!item) return;
  checkbox.checked = true;
  show(item.getAsFile());
  status.textContent = "Pasted image attached.";
});

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  const message = textarea.value.trim();
  if (!message || send.disabled) return;
  send.disabled = true;
  status.textContent = "Sending...";
  let shot = null;
  if (checkbox.checked && image) {
    const response = await fetch(`${BASE}/shot`, { method: "POST", body: image }).catch(() => null);
    if (response?.ok) shot = (await response.json()).shot;
  }
  const response = await fetch(`${BASE}/feedback`, {
    method: "POST",
    body: JSON.stringify({
      sid, page: page(), message, screen: `${innerWidth}x${innerHeight}`, shot, element: pinned?.info ?? null,
    }),
  }).catch(() => null);
  send.disabled = false;
  if (!response?.ok) {
    status.textContent = "Could not send it. Try again?";
    return;
  }
  textarea.value = "";
  pinned = null;
  pinRow.hidden = true;
  if (!phone) textarea.focus();
  status.textContent = shot ? "Sent with its image. Anything else?" : "Sent. Anything else?";
  // The next note deserves a fresh picture of the page.
  if (checkbox.checked) {
    show(null);
    retake().then(() => {
      status.textContent = shot ? "Sent with its image. Anything else?" : "Sent. Anything else?";
    });
  }
});
