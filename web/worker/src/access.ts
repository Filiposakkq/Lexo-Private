import type { Env } from "./env.ts";
import { safeEqual } from "./util.ts";

/* Who may use the admin.
   - Cloudflare Access (recommended): Cloudflare lets only your e-mail through to /admin and adds a signed token to every request
     (the header Cf-Access-Jwt-Assertion). The server checks that signature itself, so a switched-off or wrong Access setting
     leaves the admin locked, never open. Needs ACCESS_TEAM_DOMAIN and ACCESS_AUD.
   - ADMIN_TOKEN (optional emergency door): a long password typed into the admin page. Not set = this door does not exist. */

interface Jwk { kid: string; kty: string; n: string; e: string; alg?: string }
let certs: { at: number; team: string; keys: Jwk[] } | null = null;

const b64 = (s: string) => {
  const t = s.replace(/-/g, "+").replace(/_/g, "/");
  return Uint8Array.from(atob(t + "=".repeat((4 - (t.length % 4)) % 4)), (c) => c.charCodeAt(0));
};

async function keys(env: Env, now: number): Promise<Jwk[]> {
  if (env.ACCESS_JWKS) return (JSON.parse(env.ACCESS_JWKS) as { keys: Jwk[] }).keys; // tests and special cases
  const team = env.ACCESS_TEAM_DOMAIN!;
  if (certs && certs.team === team && now - certs.at < 3600) return certs.keys;
  const r = await fetch(`https://${team}/cdn-cgi/access/certs`);
  if (!r.ok) throw new Error("access_certs");
  const j = (await r.json()) as { keys: Jwk[] };
  certs = { at: now, team, keys: j.keys };
  return j.keys;
}

/** The e-mail of a valid Cloudflare Access token for this application, or null. */
export async function accessEmail(req: Request, env: Env, now: number): Promise<string | null> {
  if (!env.ACCESS_TEAM_DOMAIN || !env.ACCESS_AUD) return null;
  const jwt = req.headers.get("cf-access-jwt-assertion") ?? "";
  const parts = jwt.split(".");
  if (parts.length !== 3) return null;
  try {
    const head = JSON.parse(new TextDecoder().decode(b64(parts[0]!))) as { kid?: string; alg?: string };
    const body = JSON.parse(new TextDecoder().decode(b64(parts[1]!))) as { aud?: string | string[]; iss?: string; exp?: number; nbf?: number; email?: string };
    if (head.alg !== "RS256") return null;
    const jwk = (await keys(env, now)).find((k) => k.kid === head.kid);
    if (!jwk) return null;
    const key = await crypto.subtle.importKey("jwk", { kty: "RSA", n: jwk.n, e: jwk.e, alg: "RS256", ext: true }, { name: "RSASSA-PKCS1-v1_5", hash: "SHA-256" }, false, ["verify"]);
    const ok = await crypto.subtle.verify("RSASSA-PKCS1-v1_5", key, b64(parts[2]!), new TextEncoder().encode(parts[0] + "." + parts[1]));
    if (!ok) return null;
    const aud = Array.isArray(body.aud) ? body.aud : [body.aud];
    if (!aud.includes(env.ACCESS_AUD)) return null;
    if (body.iss !== `https://${env.ACCESS_TEAM_DOMAIN}`) return null;
    if (!body.exp || body.exp < now || (body.nbf && body.nbf > now + 60)) return null;
    return body.email ?? "access";
  } catch {
    return null;
  }
}

export const tokenOk = (req: Request, env: Env) => {
  const given = (req.headers.get("authorization") ?? "").replace(/^Bearer\s+/i, "");
  return !!env.ADMIN_TOKEN && env.ADMIN_TOKEN.length >= 16 && !!given && safeEqual(given, env.ADMIN_TOKEN);
};

export const adminConfigured = (env: Env) => !!(env.ACCESS_TEAM_DOMAIN && env.ACCESS_AUD) || (!!env.ADMIN_TOKEN && env.ADMIN_TOKEN.length >= 16);

/** Who got in: the Access e-mail, "token" for the emergency token, or null. */
export async function admitted(req: Request, env: Env, now: number): Promise<string | null> {
  const email = await accessEmail(req, env, now);
  if (email) return email;
  if (tokenOk(req, env)) return "token";
  return null;
}
