// A lost licence key, handled by the user in the application.
//
// The key is stored only as a hash, so it cannot be sent again: the user gets a NEW key and the old one stops working. To make sure a stranger cannot do that to someone else, it takes two
// steps: `request` mails a short code to the address of the purchase (nothing changes yet), `confirm` takes the code and only then replaces the key. Limits: 1 code per hour and 3 per day
// for an e-mail address, 15 minutes and 5 attempts per code, and one recovery per licence for good (the next one goes through the owner). `request` answers the same whatever the address is,
// so the call cannot be used to find out who a customer is.
import { sha256Hex } from "./crypto.ts";
import type { Env } from "./env.ts";
import { body, clean, fail, json, str } from "./handlers.ts";
import { keyHash, newKey } from "./licence.ts";
import { mailCode, mailKey } from "./mail.ts";
import * as store from "./store.ts";
import type { LicenceRow } from "./licence.ts";

export const CODE_MINUTES = 15;
export const CODE_ATTEMPTS = 5;
export const PER_HOUR = 1;
export const PER_DAY = 3;

const ALPHABET = "23456789ABCDEFGHJKMNPQRSTVWXYZ";
const newCode = (): string => {
  const t = [...crypto.getRandomValues(new Uint8Array(8))].map((b) => ALPHABET[b % ALPHABET.length]).join("");
  return `${t.slice(0, 4)}-${t.slice(4)}`;
};
const normalizeCode = (c: string) => c.toUpperCase().replace(/[^A-Z0-9]/g, "");
const codeHash = (licenceId: string, code: string) => sha256Hex(`aside-code:${licenceId}:${normalizeCode(code)}`);
const emailHash = (email: string) => sha256Hex("aside-recover:" + email);
const cleanEmail = (v: unknown) => clean(v, 200).toLowerCase();

/** The licence a recovery is about: the newest active licence bought with this address. */
async function licenceOf(env: Env, email: string): Promise<LicenceRow | null> {
  return env.DB.prepare("SELECT * FROM licences WHERE lower(email) = ? AND state = 'active' ORDER BY created DESC LIMIT 1").bind(email).first<LicenceRow>();
}

const alreadyRecovered = async (env: Env, id: string) =>
  ((await env.DB.prepare("SELECT COUNT(*) AS n FROM events WHERE licence_id = ? AND kind = 'key-recovered'").bind(id).first<{ n: number }>())?.n ?? 0) > 0;

/** `recover/request {email}`: mails a code to the address of a licence; the answer is `ok` for every address. */
export async function recoverRequest(req: Request, env: Env, now: number, defer: (work: Promise<unknown>) => void = () => {}): Promise<Response> {
  const b = await body(req);
  const email = cleanEmail(b?.email);
  if (!b || !/^[^@\s]+@[^@\s]+$/.test(email)) return fail("bad_request", 400);
  const h = await emailHash(email);
  const hour = (await env.DB.prepare("SELECT COUNT(*) AS n, MIN(at) AS oldest FROM recovery_requests WHERE email_hash = ? AND at > ?").bind(h, now - 3600).first<{ n: number; oldest: number | null }>()) ?? { n: 0, oldest: null };
  const day = (await env.DB.prepare("SELECT COUNT(*) AS n, MIN(at) AS oldest FROM recovery_requests WHERE email_hash = ? AND at > ?").bind(h, now - 86400).first<{ n: number; oldest: number | null }>()) ?? { n: 0, oldest: null };
  if (hour.n >= PER_HOUR || day.n >= PER_DAY) {
    const next = hour.n >= PER_HOUR ? (hour.oldest ?? now) + 3600 : (day.oldest ?? now) + 86400;
    return fail("too_many", 429, { next, minutes: Math.max(1, Math.ceil((next - now) / 60)) });
  }
  await env.DB.prepare("INSERT INTO recovery_requests (email_hash, at) VALUES (?,?)").bind(h, now).run();
  const l = await licenceOf(env, email);
  if (l) {
    if (await alreadyRecovered(env, l.id)) {
      await store.logEvent(env.DB, now, l.id, "recover-refused", "already used");
      defer(mailCode(env, email, null));
    } else {
      const code = newCode();
      await env.DB.prepare("INSERT OR REPLACE INTO recovery_codes (licence_id, code_hash, expires, attempts) VALUES (?,?,?,0)").bind(l.id, await codeHash(l.id, code), now + CODE_MINUTES * 60).run();
      await store.logEvent(env.DB, now, l.id, "recover-request", "");
      defer(mailCode(env, email, code));
    }
  }
  return json({ ok: true, minutes: CODE_MINUTES });
}

/** `recover/confirm {email, code}`: with the right code the key is replaced; the new key is returned and mailed. The computers that have a licence file keep working. */
export async function recoverConfirm(req: Request, env: Env, now: number): Promise<Response> {
  const b = await body(req);
  const email = cleanEmail(b?.email);
  const code = str(b?.code);
  if (!b || !email || !normalizeCode(code)) return fail("bad_request", 400);
  const l = await licenceOf(env, email);
  if (!l) return fail("invalid_code", 400);
  // an attempt is counted in one statement BEFORE the code is compared: parallel guesses cannot all pass as the first one
  const counted = await env.DB.prepare("UPDATE recovery_codes SET attempts = attempts + 1 WHERE licence_id = ? AND attempts < ? AND expires >= ?").bind(l.id, CODE_ATTEMPTS, now).run();
  if (!counted.meta.changes) return fail("invalid_code", 400);
  const row = await env.DB.prepare("SELECT code_hash FROM recovery_codes WHERE licence_id = ?").bind(l.id).first<{ code_hash: string }>();
  if (!row || (await codeHash(l.id, code)) !== row.code_hash) return fail("invalid_code", 400);
  if (await alreadyRecovered(env, l.id)) return fail("already_used", 403);
  const key = newKey();
  await store.setKeyHash(env.DB, l.id, await keyHash(key));
  await env.DB.prepare("DELETE FROM recovery_codes WHERE licence_id = ?").bind(l.id).run();
  await store.logEvent(env.DB, now, l.id, "key-recovered", "");
  await mailKey(env, l.email, key, l.id);
  return json({ ok: true, key });
}
