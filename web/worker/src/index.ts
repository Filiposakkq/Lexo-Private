import { adminApi } from "./admin.ts";
import { ADMIN_PAGE } from "./adminPage.ts";
import { daily } from "./cron.ts";
import type { Env } from "./env.ts";
import * as pub from "./public.ts";
import { fail } from "./util.ts";

const now = () => Math.floor(Date.now() / 1000);

/** /api/... : the calls of the pages. Everything else that is not a file of web/ never reaches here (the assets answer it). */
async function api(req: Request, env: Env, path: string): Promise<Response> {
  const [a, b] = path.split("/").filter(Boolean);
  if (req.method === "GET") {
    switch (a) {
      case "config": return pub.config(env);
      case "roadmap": return pub.roadmap(req, env);
      case "feedback": return b ? pub.feedbackInfo(env, b, now()) : fail("not_found", 404);
      case "tester": return b ? pub.testerInfo(env, b) : fail("not_found", 404);
      case "showcase": return pub.showcaseList(env);
      case "releases": return pub.releases(env);
      case "download": return b ? pub.download(env, b) : fail("not_found", 404);
      case "health": return new Response('{"ok":true}', { headers: { "content-type": "application/json" } });
    }
  }
  if (req.method === "POST") {
    switch (a) {
      case "apply": return pub.apply(req, env, now());
      case "vote": return pub.vote(req, env, now());
      case "feedback": return b ? pub.feedbackSend(req, env, b, now()) : fail("not_found", 404);
      case "bug": return pub.bugSend(req, env, now());
      case "showcase": return pub.showcaseSend(req, env, now());
      case "contact": return pub.contact(req, env, now());
    }
  }
  return fail("not_found", 404);
}

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);
    const path = url.pathname.replace(/\/+$/, "") || "/";
    try {
      if (path.startsWith("/api/")) return await api(req, env, path.slice(5));
      if (path === "/admin" && req.method === "GET")
        return new Response(ADMIN_PAGE, {
          headers: {
            "content-type": "text/html; charset=utf-8", "cache-control": "no-store", "x-robots-tag": "noindex",
            "content-security-policy": "default-src 'self'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src https://fonts.gstatic.com; script-src 'unsafe-inline'; img-src 'self' data: blob:; frame-ancestors 'none'",
          },
        });
      if (path.startsWith("/admin/api/")) return await adminApi(req, env, now(), path.slice("/admin/api/".length));
      return env.ASSETS.fetch(req);
    } catch (e) {
      console.error("unhandled", e instanceof Error ? e.message : String(e));
      return fail("server_error", 500);
    }
  },
  async scheduled(_c: ScheduledController, env: Env, ctx: ExecutionContext): Promise<void> {
    ctx.waitUntil(daily(env, now()).then(() => undefined));
  },
} satisfies ExportedHandler<Env>;
