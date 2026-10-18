import { safeEqual } from "./crypto.ts";
import type { Env } from "./env.ts";
import { fail, json, issue, body, str } from "./handlers.ts";
import { keyHash, newKey } from "./licence.ts";
import { mailKey } from "./mail.ts";
import * as store from "./store.ts";

/** What the admin sees of a licence: everything except the hash of the key. */
const view = ({ key_hash: _hash, ...rest }: store.ListedLicence | NonNullable<Awaited<ReturnType<typeof store.byId>>>) => rest;

function authorised(req: Request, env: Env): boolean {
  const given = (req.headers.get("authorization") ?? "").replace(/^Bearer\s+/i, "");
  return env.ADMIN_TOKEN.length >= 16 && safeEqual(given, env.ADMIN_TOKEN);
}

/** `/admin/api/...`: the API of the admin page. Every call needs `Authorization: Bearer <ADMIN_TOKEN>`. */
export async function adminApi(req: Request, env: Env, now: number, path: string): Promise<Response> {
  if (!env.ADMIN_TOKEN || env.ADMIN_TOKEN.length < 16) return fail("admin_not_configured", 503);
  if (!authorised(req, env)) return fail("unauthorised", 401);
  const url = new URL(req.url);
  const parts = path.split("/").filter(Boolean); // after /admin/api
  if (req.method === "GET" && parts[0] === "overview") {
    return json({ ok: true, stats: await store.stats(env.DB, now), licences: (await store.list(env.DB, url.searchParams.get("q") ?? "")).map(view), events: await store.recentEvents(env.DB, 30) });
  }
  if (req.method === "GET" && parts[0] === "licence" && parts[1]) {
    const l = await store.byId(env.DB, parts[1]);
    if (!l) return fail("not_found", 404);
    return json({ ok: true, licence: view(l), activations: await store.activations(env.DB, l.id), events: await store.events(env.DB, l.id) });
  }
  if (req.method !== "POST") return fail("not_found", 404);
  const b = (await body(req)) ?? {};
  if (parts[0] === "issue") {
    const email = str(b.email).trim();
    const checkDays = Number(b.check_days) || 14;
    const maxDevices = Number(b.max_devices) || 2;
    const major = Number(b.major) || undefined;
    if (checkDays < 1 || checkDays > 365 || maxDevices < 1 || maxDevices > 20) return fail("bad_request", 400);
    const features = str(b.features).trim() || "all";
    const { row, key } = await issue(env, now, { email, orderRef: null, features, major, checkDays, maxDevices, note: str(b.note) });
    const mailed = b.send === true ? await mailKey(env, email, key, row.id) : false;
    return json({ ok: true, licence: view(row), key, mailed });
  }
  if (parts[0] === "licence" && parts[1] && parts[2]) {
    const l = await store.byId(env.DB, parts[1]);
    if (!l) return fail("not_found", 404);
    switch (parts[2]) {
      case "revoke":
        await store.setState(env.DB, l.id, "revoked");
        await store.logEvent(env.DB, now, l.id, "revoked", str(b.reason) || "by the owner");
        return json({ ok: true });
      case "restore":
        await store.setState(env.DB, l.id, "active");
        await store.logEvent(env.DB, now, l.id, "restored", "by the owner");
        return json({ ok: true });
      case "release":
        await store.release(env.DB, l.id, str(b.device), now);
        await store.logEvent(env.DB, now, l.id, "released", "by the owner");
        return json({ ok: true });
      case "check_days": {
        const days = Number(b.days);
        if (!(days >= 1 && days <= 365)) return fail("bad_request", 400);
        await store.setCheckDays(env.DB, l.id, Math.round(days));
        await store.logEvent(env.DB, now, l.id, "check-days", String(Math.round(days)));
        return json({ ok: true });
      }
      case "rekey": {
        // the old key stops working at once; the new one is shown once (and mailed when asked)
        const key = newKey();
        await store.setKeyHash(env.DB, l.id, await keyHash(key));
        await store.logEvent(env.DB, now, l.id, "rekeyed", "by the owner");
        const mailed = b.send === true ? await mailKey(env, l.email, key, l.id) : false;
        return json({ ok: true, key, mailed });
      }
    }
  }
  return fail("not_found", 404);
}

