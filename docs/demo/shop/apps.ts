// The demo recordings' apps: Verdure, a React shop served by Vite in
// development on 5174 (so pointed elements carry their source file), and its
// API on 3000.
import react from "@vitejs/plugin-react";
import { createServer } from "vite";

const SHOP_PORT = 5174;
const API_PORT = 3000;
const SLOTS = ["Thursday 18:00-20:00", "Thursday 20:00-22:00"];

const shop = await createServer({
  configFile: false,
  // Its own folder, node_modules included: no module is served under this
  // computer's full path.
  root: import.meta.dir,
  plugins: [react()],
  // Visitors come through the tunnel's host name.
  server: { port: SHOP_PORT, strictPort: true, allowedHosts: true },
});
await shop.listen();

Bun.serve({
  port: API_PORT,
  async fetch(req) {
    const { pathname } = new URL(req.url);
    if (pathname === "/api/basket") return Response.json({ lines: 3, total: 57.5 });
    if (pathname === "/api/slots") return Response.json({ slots: SLOTS });
    if (pathname === "/api/items") return Response.json({ items: ["carrot", "lettuce", "radish"] });
    if (pathname === "/api/orders" && req.method === "POST") {
      await req.text();
      return Response.json({ error: "payment provider timed out" }, { status: 500 });
    }
    if (pathname === "/boom") return new Response("boom", { status: 500 });
    return new Response("not found", { status: 404 });
  },
});
console.log(`shop on http://localhost:${SHOP_PORT}, API on http://localhost:${API_PORT}`);
