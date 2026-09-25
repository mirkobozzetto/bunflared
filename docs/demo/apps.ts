// Two tiny apps for the demo recordings: a frontend on 5174 and its API on 3000.
Bun.serve({
  port: 5174,
  fetch(req) {
    const { pathname } = new URL(req.url);
    if (pathname === "/missing") return new Response("nope", { status: 404 });
    return new Response(
      `<!doctype html><title>Carrot shop</title><h1>Carrot shop</h1>
<script>fetch("http://localhost:3000/api/items")</script>`,
      { headers: { "content-type": "text/html" } },
    );
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
