// A shadow root keeps the app's CSS and this widget's CSS apart.
const host = document.createElement("bunflared-feedback");
const root = host.attachShadow({ mode: "open" });
const mac = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
const phone = matchMedia("(pointer: coarse)").matches;
const REACTIONS = [["👍", "a thumbs up"], ["🔥", "a fire"], ["😍", "heart eyes"], ["😕", "a confused face"]];
const svg = (body) => `<svg viewBox="0 0 24 24" aria-hidden="true">${body}</svg>`;
const ICON = {
  bubble: svg(`<path d="M6.8 4h10.4A2.8 2.8 0 0 1 20 6.8v6.4a2.8 2.8 0 0 1-2.8 2.8H11l-4.2 3.6V16A2.8 2.8 0 0 1 4 13.2V6.8A2.8 2.8 0 0 1 6.8 4Z"/><path d="M8.5 8.8h7M8.5 11.8h4.5"/>`),
  chevron: svg(`<path d="M6 9l6 6 6-6"/>`),
  close: svg(`<path d="M6.5 6.5l11 11M17.5 6.5l-11 11"/>`),
  point: svg(`<circle cx="12" cy="12" r="6.5"/><path d="M12 2.5v4M12 17.5v4M2.5 12h4M17.5 12h4"/><circle cx="12" cy="12" r="1.2" fill="currentColor" stroke="none"/>`),
  camera: svg(`<path d="M4 8.6A2.6 2.6 0 0 1 6.6 6h1.6l1.4-2h4.8l1.4 2h1.6A2.6 2.6 0 0 1 20 8.6v7.8a2.6 2.6 0 0 1-2.6 2.6H6.6A2.6 2.6 0 0 1 4 16.4Z"/><circle cx="12" cy="12.4" r="3.2"/>`),
  send: svg(`<path d="M12 18.5v-13M6.5 11 12 5.5l5.5 5.5"/>`),
  check: svg(`<path d="M5 12.5l4.5 4.5L19 7.5"/>`),
  alert: svg(`<circle cx="12" cy="12" r="8.5"/><path d="M12 7.8v5M12 16.2v.01"/>`),
  pin: svg(`<path d="M12 21s-6-5.6-6-10.5a6 6 0 1 1 12 0C18 15.4 12 21 12 21Z"/><circle cx="12" cy="10.5" r="2"/>`),
  hide: svg(`<path d="M3.5 3.5l17 17"/><path d="M10.6 5.2A9.6 9.6 0 0 1 12 5c5 0 8.5 4.6 9.5 7-.4 1-1.2 2.2-2.3 3.3M6.6 6.6C4.6 7.9 3.2 9.9 2.5 12c1 2.4 4.5 7 9.5 7 1.8 0 3.4-.5 4.8-1.4"/><path d="M9.9 9.9a3 3 0 0 0 4.2 4.2"/>`),
};
root.innerHTML = `
  <style>${STYLE}</style>
  <button class="tab right" type="button" aria-label="Talk to the developer" aria-haspopup="dialog"
    aria-expanded="false">${ICON.bubble}<span class="badge" aria-hidden="true" hidden></span><span
    class="dot" aria-hidden="true" hidden></span></button>
  <button class="sliver right" type="button" aria-label="Show the feedback tab" hidden></button>
  <button class="toast" type="button" hidden><b>From the developer</b><span></span></button>
  <p class="picking" role="status" hidden>${ICON.point}<span>${phone ? "Tap" : "Click"} what you mean</span>
    <button class="stop" type="button">Cancel</button></p>
  <div class="scrim" hidden></div>
  <div class="panel ${phone ? "sheet" : "pop"}" id="panel" role="dialog" aria-modal="${phone}" aria-labelledby="title" hidden>
    ${phone ? `<div class="grab" aria-hidden="true"></div>` : ""}
    <div class="head">
      <h2 id="title">Talk to the developer</h2>
      <div class="actions">
        <button class="ghost hide-tab" type="button">${ICON.hide}Hide tab</button>
        <button class="icon close" type="button" aria-label="Close">${ICON.close}</button>
      </div>
    </div>
    <p class="watch" hidden><span class="dot" aria-hidden="true"></span>The developer sees your pointer</p>
    <section class="msgs" aria-labelledby="from" hidden>
      <h3><button class="fold" id="from" type="button" aria-expanded="true" aria-controls="thread">${ICON.chevron}From the developer</button></h3>
      <ol class="thread" id="thread" role="log" aria-live="polite"></ol>
      <form class="reply">
        <label class="sr" for="answer">Reply to the developer</label>
        <input id="answer" placeholder="Reply…" maxlength="2000" autocomplete="off">
        <button class="icon" type="submit" aria-label="Send the reply">${ICON.send}</button>
      </form>
    </section>
    <div class="react" role="group" aria-label="Quick reaction" hidden>
      ${REACTIONS.map(([emoji, name]) => `<button type="button" data-emoji="${emoji}"
        aria-label="Send ${name} to the developer">${emoji}</button>`).join("")}
    </div>
    <form class="note">
      <label for="note">What should change?</label>
      <p class="pin" hidden>${ICON.pin}<span></span>
        <button class="icon unpin" type="button" aria-label="Forget this element">${ICON.close}</button></p>
      <textarea id="note" rows="3"
        placeholder="${phone ? "One remark at a time" : "One remark at a time. Paste an image to attach it."}"></textarea>
      <div class="tools">
        <button class="chip point" type="button" aria-pressed="false">${ICON.point}Point at it</button>
        <button class="chip shot" type="button" aria-pressed="true" aria-label="Attach a screenshot"><img class="thumb"
          alt="" hidden>${ICON.camera}Screenshot</button>
        <button class="send" type="submit" disabled>${ICON.send}Send</button>
      </div>
      <div class="foot"><p class="status" role="status"></p><span class="hint">${mac ? "⌘" : "Ctrl"} Enter</span></div>
    </form>
  </div>
  <p class="sr say" role="status"></p>`;

const $ = (selector) => root.querySelector(selector);

// Read out by screen readers, even when the same words come twice.
const announce = (text) => {
  const line = $(".say");
  line.textContent = "";
  setTimeout(() => { line.textContent = text; }, 50);
};
