// Two tiny apps for the demo recordings: a carrot shop on 5174 and its API on 3000.
const STYLE = `
  body { margin: 0; font: 17px/1.5 system-ui, sans-serif; background: #fff8ef; color: #2b2118; }
  header { display: flex; gap: 28px; align-items: center; padding: 18px 48px; background: #ff8a3d; color: #fff; }
  header strong { font-size: 24px; margin-right: auto; }
  header a { color: #fff; text-decoration: none; font-weight: 600; }
  main { padding: 28px 48px; }
  h1 { margin: 0 0 20px; }
  .grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 24px; }
  .card { background: #fff; border-radius: 18px; padding: 24px; box-shadow: 0 6px 20px rgb(0 0 0 / 8%); }
  .card b { display: block; font-size: 56px; }
  button { font: inherit; font-weight: 700; border: 0; border-radius: 999px; padding: 10px 22px;
    background: #2b2118; color: #fff; white-space: nowrap; }
  .basket { max-width: 460px; display: grid; gap: 12px; }
  .line { display: flex; justify-content: space-between; }
  /* The bug the demo is about: the pay button does not fit. */
  .pay { width: 110px; overflow: hidden; }
  .pay button { background: #ff8a3d; }
`;

const page = (title: string, body: string) =>
  new Response(
    `<!doctype html><meta name="viewport" content="width=device-width">
<title>${title}</title><style>${STYLE}</style>
<header><strong>🥕 Carrot shop</strong><a href="/">Shop</a><a href="/checkout">Checkout</a></header>
<main>${body}</main>
<script>fetch("http://localhost:3000/api/items")</script>`,
    { headers: { "content-type": "text/html; charset=utf-8" } },
  );

const SHOP = `<h1>Fresh from the garden</h1><div class="grid">
${[["🥕", "Carrots", "2 €"], ["🥬", "Lettuce", "3 €"], ["🌱", "Radishes", "1 €"]]
  .map(([icon, name, price]) => `<div class="card"><b>${icon}</b><h3>${name}</h3><p>${price} a bunch</p><button>Add to basket</button></div>`)
  .join("")}</div>`;

const CHECKOUT = `<h1>Your basket</h1><div class="card basket">
<div class="line"><span>🥕 Carrots × 2</span><span>4 €</span></div>
<div class="line"><span>🌱 Radishes × 2</span><span>2 €</span></div>
<div class="line"><strong>Total</strong><strong>6 €</strong></div>
<div class="pay"><button>Pay 6 € now</button></div></div>`;

Bun.serve({
  port: 5174,
  fetch(req) {
    const { pathname } = new URL(req.url);
    if (pathname === "/") return page("Carrot shop", SHOP);
    if (pathname === "/checkout") return page("Checkout", CHECKOUT);
    return new Response("nope", { status: 404 });
  },
});
Bun.serve({
  port: 3000,
  fetch(req) {
    const { pathname } = new URL(req.url);
    if (pathname === "/boom") return new Response("boom", { status: 500 });
    return Response.json({ items: ["carrot", "lettuce", "radish"] });
  },
});
