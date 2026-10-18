import type { Env } from "./env.ts";

const enc = new TextEncoder();

export const json = (body: unknown, status = 200, headers: Record<string, string> = {}): Response =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store", ...headers } });

export const fail = (error: string, status: number, extra: Record<string, unknown> = {}) => json({ ok: false, error, ...extra }, status);

export async function body(req: Request, max = 2_000_000): Promise<Record<string, unknown> | null> {
  const len = Number(req.headers.get("content-length") ?? 0);
  if (len > max) return null;
  try {
    const text = await req.text();
    if (text.length > max) return null;
    const v = JSON.parse(text);
    return v && typeof v === "object" && !Array.isArray(v) ? (v as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

export const str = (v: unknown) => (typeof v === "string" ? v : "");

/** A one-line text from a visitor: no control characters, trimmed, limited. */
export const line = (v: unknown, max: number) => str(v).replace(/[\u0000-\u001f\u007f]/g, " ").replace(/\s+/g, " ").trim().slice(0, max);

/** A longer text from a visitor: keeps line breaks, drops other control characters. */
export const text = (v: unknown, max: number) => str(v).replace(/\r\n?/g, "\n").replace(/[\u0000-\u0009\u000b-\u001f\u007f]/g, " ").trim().slice(0, max);

export const isEmail = (s: string) => /^[^\s@]+@[^\s@]+\.[^\s@]{2,}$/.test(s) && s.length <= 200;
export const isUrl = (s: string) => /^https?:\/\/[^\s/$.?#][^\s]*\.[^\s]+$/i.test(s) && s.length <= 300;

export async function sha256Hex(t: string): Promise<string> {
  const d = await crypto.subtle.digest("SHA-256", enc.encode(t));
  return [...new Uint8Array(d)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/** Compares two strings without stopping at the first difference. */
export function safeEqual(a: string, b: string): boolean {
  const x = enc.encode(a);
  const y = enc.encode(b);
  let diff = x.length ^ y.length;
  for (let i = 0; i < Math.max(x.length, y.length); i++) diff |= (x[i] ?? 0) ^ (y[i] ?? 0);
  return diff === 0;
}

const ALPHA = "abcdefghjkmnpqrstuvwxyz23456789";
/** A random id or secret code from an alphabet without look-alike characters. */
export function randomId(n = 10): string {
  const bytes = crypto.getRandomValues(new Uint8Array(n));
  return [...bytes].map((b) => ALPHA[b % ALPHA.length]).join("");
}

/** The address of the visitor, hashed with a server secret (the address itself is never stored). */
export async function ipHash(req: Request, env: Env): Promise<string> {
  const ip = req.headers.get("cf-connecting-ip") ?? "local";
  return (await sha256Hex("ip:" + env.ADMIN_TOKEN + ":" + ip)).slice(0, 32);
}

/** Counts a request of a kind and says whether the visitor is still within `limit` requests per `seconds`. */
export async function within(env: Env, hash: string, kind: string, limit: number, seconds: number, now: number): Promise<boolean> {
  const r = await env.DB.prepare("SELECT COUNT(*) AS n FROM hits WHERE kind = ? AND ip_hash = ? AND at > ?").bind(kind, hash, now - seconds).first<{ n: number }>();
  if ((r?.n ?? 0) >= limit) return false;
  await env.DB.prepare("INSERT INTO hits (ip_hash, kind, at) VALUES (?, ?, ?)").bind(hash, kind, now).run();
  return true;
}

/** Checks the Turnstile answer of a form. Without TURNSTILE_SECRET every form passes (development, or before Turnstile is set up). */
export async function human(env: Env, token: unknown, req: Request): Promise<boolean> {
  if (!env.TURNSTILE_SECRET) return true;
  const t = str(token);
  if (!t) return false;
  try {
    const form = new FormData();
    form.append("secret", env.TURNSTILE_SECRET);
    form.append("response", t);
    const ip = req.headers.get("cf-connecting-ip");
    if (ip) form.append("remoteip", ip);
    const r = await fetch("https://challenges.cloudflare.com/turnstile/v0/siteverify", { method: "POST", body: form });
    const j = (await r.json()) as { success?: boolean };
    return j.success === true;
  } catch {
    return false;
  }
}

export const site = (env: Env) => (env.SITE_URL ?? "https://aside.filiposakkq.dev").replace(/\/+$/, "");
