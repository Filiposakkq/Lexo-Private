import { sha256Hex } from "./crypto.ts";

export interface LicenceRow {
  id: string;
  key_hash: string;
  email: string;
  order_ref: string | null;
  major: number;
  features: string;
  check_days: number;
  max_devices: number;
  state: string;
  created: number;
  note: string;
}

// Crockford base32 without the look-alike letters: easy to read out and to type.
const ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

function randomText(length: number): string {
  const bytes = crypto.getRandomValues(new Uint8Array(length));
  return [...bytes].map((b) => ALPHABET[b % 32]).join("");
}

/** A new licence id, for the owner and the logs (not a secret). */
export function newLicenceId(): string {
  return "L-" + randomText(8);
}

/** A new licence key: 100 bits of randomness, `ASIDE-XXXXX-XXXXX-XXXXX-XXXXX`. */
export function newKey(): string {
  const t = randomText(20);
  return `ASIDE-${t.slice(0, 5)}-${t.slice(5, 10)}-${t.slice(10, 15)}-${t.slice(15)}`;
}

/** The form that is hashed and compared: upper case, no spaces, only letters and digits and dashes. */
export function normalizeKey(text: string): string {
  return text.toUpperCase().replace(/[^A-Z0-9-]/g, "");
}

export async function keyHash(key: string): Promise<string> {
  return sha256Hex("aside-key:" + normalizeKey(key));
}

/** The payload of a licence file (`pro/rust/lexo-pro/src/licence.rs`): `device` is the device it is made for. */
export function payloadFor(l: LicenceRow, device: string, now: number): Record<string, unknown> {
  return {
    v: 1,
    id: l.id,
    features: l.features === "all" ? ["all"] : l.features.split(",").map((f) => f.trim()).filter(Boolean),
    major: l.major,
    device,
    issued: now,
    check_days: l.check_days,
  };
}

export const DEVICE_RE = /^[0-9a-f]{16,64}$/;

/** The id of the key this server signs with. */
export const kidOf = (env: { LICENCE_KID?: string }) => (env.LICENCE_KID || "k1").trim();

/** The public keys a licence file may have been signed with: the current one and the older ones. */
export function trustedKeys(env: { LICENCE_KID?: string; LICENCE_PUBLIC_KEY: string; LICENCE_OLD_PUBLIC_KEYS?: string }): Record<string, string> {
  let old: Record<string, string> = {};
  try {
    const v = JSON.parse(env.LICENCE_OLD_PUBLIC_KEYS || "{}");
    if (v && typeof v === "object") old = v as Record<string, string>;
  } catch {
    /* a broken value trusts nothing extra */
  }
  return { ...old, [kidOf(env)]: env.LICENCE_PUBLIC_KEY };
}
