import { adminApi } from "./admin.ts";
import { ADMIN_PAGE } from "./adminPage.ts";
import type { Env } from "./env.ts";
import { activate, deactivate, fail, parseLemonSqueezy, refresh, webhook } from "./handlers.ts";

const now = () => Math.floor(Date.now() / 1000);

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);
    const path = url.pathname.replace(/\/+$/, "") || "/";
    try {
      if (req.method === "GET" && path === "/admin") {
        return new Response(ADMIN_PAGE, {
          headers: { "content-type": "text/html; charset=utf-8", "cache-control": "no-store", "x-robots-tag": "noindex", "content-security-policy": "default-src 'self'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src data:; frame-ancestors 'none'" },
        });
      }
      if (path.startsWith("/admin/api/")) return await adminApi(req, env, now(), path.slice("/admin/api/".length));
      if (req.method === "GET" && path === "/v1/health") return new Response(JSON.stringify({ ok: true }), { headers: { "content-type": "application/json" } });
      if (req.method === "POST") {
        switch (path) {
          case "/v1/activate":
            return await activate(req, env, now());
          case "/v1/refresh":
            return await refresh(req, env, now());
          case "/v1/deactivate":
            return await deactivate(req, env, now());
          case "/v1/webhook/lemonsqueezy":
            return await webhook(req, env, now(), parseLemonSqueezy);
        }
      }
      return fail("not_found", 404);
    } catch (e) {
      console.error("unhandled", e instanceof Error ? e.message : String(e));
      return fail("server_error", 500);
    }
  },
} satisfies ExportedHandler<Env>;
