// The cryptography of the server: hashes, the webhook signature and the Ed25519 signature of licence files (the same format the Pro pack checks: pro/rust/lexo-pro/src/licence.rs).
const enc = new TextEncoder();

export function toHex(data: ArrayBuffer | Uint8Array): string {
  const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);
  return [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");
}

export function fromHex(hex: string): Uint8Array {
  if (hex.length % 2 !== 0 || !/^[0-9a-fA-F]*$/.test(hex)) throw new Error("not hex");
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  return out;
}

export function b64url(data: ArrayBuffer | Uint8Array): string {
  const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

export function b64urlDecode(text: string): Uint8Array {
  const s = text.replace(/-/g, "+").replace(/_/g, "/");
  const bin = atob(s + "=".repeat((4 - (s.length % 4)) % 4));
  return Uint8Array.from(bin, (c) => c.charCodeAt(0));
}

export async function sha256Hex(text: string): Promise<string> {
  return toHex(await crypto.subtle.digest("SHA-256", enc.encode(text)));
}

export async function hmacSha256Hex(secret: string, data: string): Promise<string> {
  const key = await crypto.subtle.importKey("raw", enc.encode(secret), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  return toHex(await crypto.subtle.sign("HMAC", key, enc.encode(data)));
}

/** Compares two strings without stopping at the first difference. */
export function safeEqual(a: string, b: string): boolean {
  const x = enc.encode(a);
  const y = enc.encode(b);
  let diff = x.length ^ y.length;
  for (let i = 0; i < Math.max(x.length, y.length); i++) diff |= (x[i] ?? 0) ^ (y[i] ?? 0);
  return diff === 0;
}

// PKCS#8 wrapper of a raw Ed25519 seed: WebCrypto only imports private keys in this form.
const PKCS8_ED25519 = fromHex("302e020100300506032b657004220420");

export interface LicenceFile {
  payload: string;
  sig: string;
}

/** Signs a payload: the signature covers the exact bytes that are base64url-encoded into `payload`. */
export async function signFile(seedHex: string, payload: object): Promise<LicenceFile> {
  const der = new Uint8Array(PKCS8_ED25519.length + 32);
  der.set(PKCS8_ED25519);
  der.set(fromHex(seedHex), PKCS8_ED25519.length);
  const key = await crypto.subtle.importKey("pkcs8", der, { name: "Ed25519" }, false, ["sign"]);
  const bytes = enc.encode(JSON.stringify(payload));
  return { payload: b64url(bytes), sig: b64url(await crypto.subtle.sign({ name: "Ed25519" }, key, bytes)) };
}

/** The payload of a licence file if its signature is valid, otherwise `null`. */
export async function verifyFile(publicHex: string, file: unknown): Promise<Record<string, unknown> | null> {
  try {
    const f = file as Partial<LicenceFile>;
    if (typeof f?.payload !== "string" || typeof f?.sig !== "string") return null;
    const key = await crypto.subtle.importKey("raw", fromHex(publicHex), { name: "Ed25519" }, false, ["verify"]);
    const bytes = b64urlDecode(f.payload);
    if (!(await crypto.subtle.verify({ name: "Ed25519" }, key, b64urlDecode(f.sig), bytes))) return null;
    return JSON.parse(new TextDecoder().decode(bytes)) as Record<string, unknown>;
  } catch {
    return null;
  }
}

/** The payload of a licence file made by this server: the key id it names is looked up in `keys` ({kid: public key hex}); a file without `kid` was signed by `k1`. */
export async function verifyFileWithKeys(keys: Record<string, string>, file: unknown): Promise<Record<string, unknown> | null> {
  try {
    const f = file as Partial<LicenceFile>;
    if (typeof f?.payload !== "string") return null;
    const named = (JSON.parse(new TextDecoder().decode(b64urlDecode(f.payload))) as { kid?: unknown }).kid;
    const kid = typeof named === "string" ? named : "k1";
    const hex = Object.hasOwn(keys, kid) ? keys[kid] : undefined;
    return hex ? await verifyFile(hex, file) : null;
  } catch {
    return null;
  }
}
