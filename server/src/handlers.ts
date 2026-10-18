import { hmacSha256Hex, safeEqual, signFile, verifyFile } from "./crypto.ts";
import type { Env } from "./env.ts";
import { DEVICE_RE, keyHash, newKey, newLicenceId, payloadFor, type LicenceRow } from "./licence.ts";
import { mailKey } from "./mail.ts";
import * as store from "./store.ts";

export const json = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store" } });

export const fail = (error: string, status: number, extra: Record<string, unknown> = {}) => json({ ok: false, error, ...extra }, status);

export async function body(req: Request): Promise<Record<string, unknown> | null> {
  try {
    const v = await req.json();
    return v && typeof v === "object" ? (v as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

export const str = (v: unknown) => (typeof v === "string" ? v : "");

// ---------------------------------------------------------------- the calls of the application

/** `activate {key, device}`: takes a place for the device (at most `max_devices`) and returns the signed licence file. */
export async function activate(req: Request, env: Env, now: number): Promise<Response> {
  const b = await body(req);
  const device = str(b?.device);
  if (!b || !DEVICE_RE.test(device) || !str(b.key)) return fail("bad_request", 400);
  const l = await store.byKeyHash(env.DB, await keyHash(str(b.key)));
  if (!l) {
    await store.logEvent(env.DB, now, null, "refused", "unknown key");
    return fail("invalid_key", 404);
  }
  if (l.state !== "active") {
    await store.logEvent(env.DB, now, l.id, "refused", "revoked key used");
    return fail("revoked", 403);
  }
  const mine = await store.activation(env.DB, l.id, device);
  if (!mine || mine.released !== null) {
    const n = await store.activeCount(env.DB, l.id);
    if (n >= l.max_devices) {
      await store.logEvent(env.DB, now, l.id, "refused", `device limit ${n}/${l.max_devices}`);
      return fail("device_limit", 409, { devices: n, max: l.max_devices });
    }
    await store.takePlace(env.DB, l.id, device, now);
    await store.logEvent(env.DB, now, l.id, "activate", device.slice(0, 8));
  } else {
    await store.touch(env.DB, l.id, device, now);
    await store.logEvent(env.DB, now, l.id, "activate-again", device.slice(0, 8));
  }
  return json({ ok: true, licence: await signFile(env.LICENCE_SIGNING_KEY, payloadFor(l, device, now)) });
}

/** The licence of a request that carries a licence file made by this server and a device: `refresh` and `deactivate` use it. */
async function ownLicence(req: Request, env: Env): Promise<{ l: LicenceRow; device: string } | Response> {
  const b = await body(req);
  const device = str(b?.device);
  if (!b || !DEVICE_RE.test(device)) return fail("bad_request", 400);
  const p = await verifyFile(env.LICENCE_PUBLIC_KEY, b.licence);
  if (!p) return fail("bad_licence", 403);
  const l = await store.byId(env.DB, str(p.id));
  if (!l) return fail("bad_licence", 403);
  if (p.device !== "any" && p.device !== device) return fail("bad_licence", 403);
  return { l, device };
}

/** `refresh {licence, device}`: the regular check; a renewed file, or a refusal for a revoked licence. */
export async function refresh(req: Request, env: Env, now: number): Promise<Response> {
  const r = await ownLicence(req, env);
  if (r instanceof Response) return r;
  const { l, device } = r;
  if (l.state !== "active") {
    await store.logEvent(env.DB, now, l.id, "refused", "refresh of a revoked licence");
    return fail("revoked", 403);
  }
  const mine = await store.activation(env.DB, l.id, device);
  if (!mine || mine.released !== null) return fail("not_activated", 403);
  await store.touch(env.DB, l.id, device, now);
  await store.logEvent(env.DB, now, l.id, "refresh", device.slice(0, 8));
  return json({ ok: true, licence: await signFile(env.LICENCE_SIGNING_KEY, payloadFor(l, device, now)) });
}

/** `deactivate {licence, device}`: frees the place of this device. */
export async function deactivate(req: Request, env: Env, now: number): Promise<Response> {
  const r = await ownLicence(req, env);
  if (r instanceof Response) return r;
  await store.release(env.DB, r.l.id, r.device, now);
  await store.logEvent(env.DB, now, r.l.id, "deactivate", r.device.slice(0, 8));
  return json({ ok: true });
}

// ---------------------------------------------------------------- the payment provider

export interface Order {
  kind: "paid" | "refunded";
  ref: string;
  email: string;
}

/** Lemon Squeezy: `meta.event_name` and `data.id` / `data.attributes`. (Check against the documentation of the provider when the account exists.) */
export function parseLemonSqueezy(payload: unknown): Order | null {
  const p = payload as { meta?: { event_name?: string }; data?: { id?: string | number; attributes?: { user_email?: string; status?: string } } };
  const event = p?.meta?.event_name;
  const ref = p?.data?.id === undefined ? "" : `ls-${p.data.id}`;
  const email = p?.data?.attributes?.user_email ?? "";
  if (!ref) return null;
  if (event === "order_created" && p.data?.attributes?.status === "paid") return { kind: "paid", ref, email };
  if (event === "order_refunded") return { kind: "refunded", ref, email };
  return null;
}

export interface IssueOptions {
  email: string;
  orderRef: string | null;
  features?: string;
  major?: number;
  checkDays?: number;
  maxDevices?: number;
  note?: string;
}

/** A new licence: returns the row and the key (the only time it exists in readable form). */
export async function issue(env: Env, now: number, o: IssueOptions): Promise<{ row: LicenceRow; key: string }> {
  const key = newKey();
  const row: LicenceRow = {
    id: newLicenceId(),
    key_hash: await keyHash(key),
    email: o.email,
    order_ref: o.orderRef,
    major: o.major ?? Number(env.PRODUCT_MAJOR ?? 1),
    features: o.features ?? "all",
    check_days: o.checkDays ?? 14,
    max_devices: o.maxDevices ?? 2,
    state: "active",
    created: now,
    note: o.note ?? "",
  };
  await store.insertLicence(env.DB, row);
  await store.logEvent(env.DB, now, row.id, "issued", o.orderRef ?? "by hand");
  return { row, key };
}

/** The webhook of the payment provider: a purchase makes a licence (once per order), a refund revokes it. */
export async function webhook(req: Request, env: Env, now: number, parse: (payload: unknown) => Order | null): Promise<Response> {
  const raw = await req.text();
  const signature = req.headers.get("x-signature") ?? "";
  if (!env.WEBHOOK_SECRET || !safeEqual(signature, await hmacSha256Hex(env.WEBHOOK_SECRET, raw))) return fail("bad_signature", 401);
  let order: Order | null;
  try {
    order = parse(JSON.parse(raw));
  } catch {
    return fail("bad_request", 400);
  }
  if (!order) return json({ ok: true, ignored: true });
  const existing = await store.byOrder(env.DB, order.ref);
  if (order.kind === "refunded") {
    if (existing && existing.state === "active") {
      await store.setState(env.DB, existing.id, "revoked");
      await store.logEvent(env.DB, now, existing.id, "revoked", "refund");
    }
    return json({ ok: true });
  }
  if (existing) return json({ ok: true, duplicate: true });
  const { row, key } = await issue(env, now, { email: order.email, orderRef: order.ref });
  const mailed = await mailKey(env, order.email, key, row.id);
  await store.logEvent(env.DB, now, row.id, mailed ? "key-mailed" : "key-not-mailed", order.email);
  return json({ ok: true, licence: row.id });
}
