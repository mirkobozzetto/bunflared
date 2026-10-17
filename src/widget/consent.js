// On a public link the visitor decides: the agent sees the page and hears its
// errors only after their yes, kept for the visit. On the developer's own
// computer nothing is asked.
const CONSENT_KEY = "bunflared-agent-sees";
const consentBar = $(".consent");
const ownComputer = ["127.0.0.1", "localhost", "[::1]"].includes(location.hostname);
const remembered = () => {
  try {
    return sessionStorage.getItem(CONSENT_KEY);
  } catch {
    return null;
  }
};
let agreed = ownComputer || remembered() === "yes";
let asking = null;
const allowed = () => {
  if (agreed) return Promise.resolve(true);
  if (remembered() === "no") return Promise.resolve(false);
  asking ??= new Promise((done) => {
    const answer = (yes) => {
      try {
        sessionStorage.setItem(CONSENT_KEY, yes ? "yes" : "no");
      } catch {}
      agreed = yes;
      consentBar.hidden = true;
      asking = null;
      done(yes);
    };
    $(".consent .allow").onclick = () => answer(true);
    $(".consent .deny").onclick = () => answer(false);
    consentBar.hidden = false;
    $(".consent .allow").focus({ preventScroll: true });
  });
  return asking;
};
