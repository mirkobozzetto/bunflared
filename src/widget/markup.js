// A shadow root keeps the app's CSS and this widget's CSS apart.
const host = document.createElement("bunflared-feedback");
const root = host.attachShadow({ mode: "open" });
const mac = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
const phone = matchMedia("(pointer: coarse)").matches;
const svg = (body) => `<svg viewBox="0 0 24 24" aria-hidden="true">${body}</svg>`;
const ICON = {
  bubble: svg(`<path d="M6.8 4h10.4A2.8 2.8 0 0 1 20 6.8v6.4a2.8 2.8 0 0 1-2.8 2.8H11l-4.2 3.6V16A2.8 2.8 0 0 1 4 13.2V6.8A2.8 2.8 0 0 1 6.8 4Z"/><path d="M8.5 8.8h7M8.5 11.8h4.5"/>`),
};
root.innerHTML = `
  <style>${STYLE}</style>
  <button class="tab right" type="button" aria-label="Talk to the developer" aria-haspopup="dialog"
    aria-expanded="false">${ICON.bubble}</button>
  <div class="dock">
  <p class="watch" hidden>Live: the developer sees your pointer</p>
  <section class="card chat" hidden>
    <header>
      <strong>From the developer</strong>
      <button class="hide" type="button" title="Hide the conversation">✕</button>
    </header>
    <ol class="thread" aria-live="polite"></ol>
    <form class="reply">
      <input placeholder="Answer..." maxlength="2000" aria-label="Your answer">
      <button class="answer" type="submit">Send</button>
    </form>
  </section>
  <div class="bar">
    <span class="reactions" hidden>
      ${REACTIONS.map((emoji) => `<button type="button" title="Send ${emoji} to the developer">${emoji}</button>`).join("")}
    </span>
  </div>
  <p class="picking" hidden>${phone ? "Tap" : "Click"} what you mean <button class="stop" type="button">Cancel</button></p>
  <form class="card note" hidden>
    <strong>What should change?</strong>
    <textarea rows="${phone ? 2 : 4}" placeholder="${phone ? "What should change?" : "One remark at a time. Paste an image to attach it."}"></textarea>
    <button class="point" type="button" title="Then click the part of the page you mean">⌖ Point at it</button>
    <p class="pin" hidden><span></span><button class="unpin" type="button" title="Forget this element">✕</button></p>
    <label><input type="checkbox" checked> Attach a screenshot of this page</label>
    <figure hidden>
      <a target="_blank" rel="noopener" title="Open it in a new tab"><img alt="Image sent with the note"></a>
      <span class="tools">
        <button class="retake" type="button" title="Take the screenshot again">↻</button>
        <button class="drop" type="button" title="Send without image">✕</button>
      </span>
    </figure>
    <p class="status" aria-live="polite"></p>
    <div class="row">
      <span class="hint">${mac ? "⌘" : "Ctrl"} Enter to send</span>
      <button class="close" type="button">Close</button>
      <button class="send" type="submit">Send</button>
    </div>
  </form>
  </div>`;

const $ = (selector) => root.querySelector(selector);
