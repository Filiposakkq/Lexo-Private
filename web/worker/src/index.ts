import { adminApi } from "./admin.ts";
import { ADMIN_PAGE } from "./adminPage.ts";
import { daily, hourly } from "./cron.ts";
import type { Env } from "./env.ts";
import * as pub from "./public.ts";
import * as prog from "./program.ts";
import { fail } from "./util.ts";

const now = () => Math.floor(Date.now() / 1000);

/** /api/... : the calls of the pages. Everything else that is not a file of web/ never reaches here (the assets answer it). */
async function api(req: Request, env: Env, path: string): Promise<Response> {
  const [a, b, c] = path.split("/").filter(Boolean);
  if (req.method === "GET") {
    switch (a) {
      case "config": return pub.config(env);
      case "roadmap": return pub.roadmap(req, env);
      case "feedback": return b ? pub.feedbackInfo(req, env, b, now()) : fail("not_found", 404);
      case "tester": return b ? pub.testerInfo(req, env, b, now()) : fail("not_found", 404);
      case "showcase": return pub.showcaseList(env);
      case "releases": return pub.releases(env);
      case "download": return b ? pub.download(env, b) : fail("not_found", 404);
      case "build": return b && c ? prog.buildDownload(req, env, b, c, now()) : fail("not_found", 404);
      case "status": return prog.status(env);
      case "faq": return prog.faq(env);
      case "posts": return prog.posts(env);
      case "subscription": return b ? prog.subscription(req, env, b, now()) : fail("not_found", 404);
      // for an outside uptime monitor: answers only when the Worker and the database work
      case "health": {
        try {
          await env.DB.prepare("SELECT 1").first();
          return new Response('{"ok":true}', { headers: { "content-type": "application/json", "cache-control": "no-store" } });
        } catch {
          return new Response('{"ok":false,"error":"database"}', { status: 503, headers: { "content-type": "application/json", "cache-control": "no-store" } });
        }
      }
    }
  }
  if (req.method === "POST") {
    switch (a) {
      case "apply": return pub.apply(req, env, now());
      case "confirm": return pub.confirm(req, env, now());
      case "vote": return pub.vote(req, env, now());
      case "feedback": return b ? pub.feedbackSend(req, env, b, now()) : fail("not_found", 404);
      case "bug": return pub.bugSend(req, env, now());
      case "showcase": return pub.showcaseSend(req, env, now());
      case "contact": return pub.contact(req, env, now());
      case "task": return prog.taskResult(req, env, now());
      case "subscribe": return b === "confirm" ? prog.subscribeConfirm(req, env, now()) : prog.subscribe(req, env, now());
      case "subscription": return b ? prog.subscription(req, env, b, now()) : fail("not_found", 404);
      case "unsubscribe": return b ? prog.oneClick(env, b) : fail("not_found", 404);
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
  async scheduled(c: ScheduledController, env: Env, ctx: ExecutionContext): Promise<void> {
    // wrangler.toml: the daily job at 08:17 UTC, the health check every hour at :07
    ctx.waitUntil((c.cron === "7 * * * *" ? hourly(env, now()) : daily(env, now())).then(() => undefined));
  },
} satisfies ExportedHandler<Env>;
