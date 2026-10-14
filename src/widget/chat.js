// The conversation with the developer, inside the panel. A message never
// covers the page: it waits as a badge and a short preview on the tab.
const THREAD_KEEP = 30;
const [msgs, thread, reply, answer] = [$(".msgs"), $(".thread"), $(".reply"), $("#answer")];

const bubble = (text, mine) => {
  const item = document.createElement("li");
  item.textContent = text;
  if (mine) item.className = "mine";
  thread.append(item);
  while (thread.children.length > THREAD_KEEP) thread.firstElementChild.remove();
  msgs.hidden = false;
  thread.scrollTop = thread.scrollHeight;
};

function heard(text) {
  bubble(text, false);
  paint();
  if (isOpen()) return;
  marks.unread += 1;
  signal();
  preview(text);
  announce(`Message from the developer: ${text}`);
}

reply.addEventListener("submit", (event) => {
  event.preventDefault();
  const text = answer.value.trim();
  if (!text) return;
  if (live?.readyState !== WebSocket.OPEN) {
    answer.placeholder = "Not connected, try again in a moment.";
    return;
  }
  say({ type: "chat", text, page: page() });
  bubble(text, true);
  answer.value = "";
});
