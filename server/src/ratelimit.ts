// Limits per network address for the calls of the application (Cloudflare's own rule is only one on the free plan, and it is used elsewhere).
//
// A counter per (hashed address, call, hour) in D1, raised in ONE statement so that parallel requests are counted one by one. The address comes from `CF-Connecting-IP`, which Cloudflare
// always sets; without it (a local run, the tests) nothing is limited. The address is hashed together with the day, and old counters are deleted, so the table does not keep who was
// where for long.
import { sha256Hex } from "./crypto.ts";
import type { Env } from "./env.ts";
import { fail } from "./handlers.ts";

/** Calls per hour for one address. A school or an office behind one address is the reason for the generous numbers of the regular calls. */
export const LIMITS: Record<string, number> = {
  "/v1/recover/request": 5,
  "/v1/recover/confirm": 20,
  "/v1/activate": 30,
  "/v1/refresh": 120,
  "/v1/deactivate": 30,
  "/v1/devices": 60,
  "/v1/release": 30,
};

const HOUR = 3600;

/** `null` when the call may go on, otherwise the answer "too many requests". */
export async function limited(req: Request, env: Env, path: string, now: number): Promise<Response | null> {
  const max = LIMITS[path];
  const ip = req.headers.get("cf-connecting-ip");
  if (!max || !ip) return null;
  const hour = Math.floor(now / HOUR);
  const bucket = await sha256Hex(`aside-ip:${Math.floor(now / 86400)}:${ip}:${path}:${hour}`);
  const row = await env.DB.prepare("INSERT INTO rate_limits (bucket, n, at) VALUES (?, 1, ?) ON CONFLICT(bucket) DO UPDATE SET n = n + 1 RETURNING n").bind(bucket, now).first<{ n: number }>();
  if (Math.random() < 0.02) await env.DB.prepare("DELETE FROM rate_limits WHERE at < ?").bind(now - 3 * HOUR).run(); // now and then, the old counters go
  if ((row?.n ?? 1) <= max) return null;
  const minutes = Math.max(1, Math.ceil(((hour + 1) * HOUR - now) / 60));
  return fail("rate_limited", 429, { minutes });
}
