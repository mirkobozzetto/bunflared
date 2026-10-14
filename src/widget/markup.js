// A shadow root keeps the app's CSS and this widget's CSS apart.
const host = document.createElement("bunflared-feedback");
const root = host.attachShadow({ mode: "open" });
const mac = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
const phone = matchMedia("(pointer: coarse)").matches;
const svg = (body) => `<svg viewBox="0 0 24 24" aria-hidden="true">${body}</svg>`;
const ICON = {
  bubble: svg(`<path d="M6.8 4h10.4A2.8 2.8 0 0 1 20 6.8v6.4a2.8 2.8 0 0 1-2.8 2.8H11l-4.2 3.6V16A2.8 2.8 0 0 1 4 13.2V6.8A2.8 2.8 0 0 1 6.8 4Z"/><path d="M8.5 8.8h7M8.5 11.8h4.5"/>`),
  close: svg(`<path d="M6.5 6.5l11 11M17.5 6.5l-11 11"/>`),
  hide: svg(`<path d="M3.5 3.5l17 17"/><path d="M10.6 5.2A9.6 9.6 0 0 1 12 5c5 0 8.5 4.6 9.5 7-.4 1-1.2 2.2-2.3 3.3M6.6 6.6C4.6 7.9 3.2 9.9 2.5 12c1 2.4 4.5 7 9.5 7 1.8 0 3.4-.5 4.8-1.4"/><path d="M9.9 9.9a3 3 0 0 0 4.2 4.2"/>`),
};
root.innerHTML = `
  <style>${STYLE}</style>
  <button class="tab right" type="button" aria-label="Talk to the developer" aria-haspopup="dialog"
    aria-expanded="false">${ICON.bubble}</button>
  <button class="sliver right" type="button" aria-label="Show the feedback tab" hidden></button>
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
  </div>
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
    <form class="note">
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
        <button class="send" type="submit">Send</button>
      </div>
    </form>
  </div>`;

const $ = (selector) => root.querySelector(selector);
