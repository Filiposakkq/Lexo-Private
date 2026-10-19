import { env, exports } from "cloudflare:workers";
import { describe, expect, it } from "vitest";
import { hmacSha256Hex, sha256Hex, signFile, verifyFile } from "../src/crypto.ts";
import type { Env } from "../src/env.ts";

type Json = Record<string, any>;
const E = env as unknown as Env;
const device = (n: number) => String(n).padStart(32, "0").replace(/0/g, "a").slice(0, 31) + (n % 10);

async function call(path: string, init: { method?: string; body?: unknown; headers?: Record<string, string> } = {}): Promise<{ status: number; json: Json }> {
  const r = await exports.default.fetch(
    new Request("https://licence.test" + path, {
      method: init.method ?? "POST",
      headers: { "content-type": "application/json", ...init.headers },
      body: init.body === undefined ? undefined : typeof init.body === "string" ? init.body : JSON.stringify(init.body),
    }),
  );
  return { status: r.status, json: (await r.json().catch(() => ({}))) as Json };
}

const admin = (path: string, body?: unknown, method = body === undefined ? "GET" : "POST") =>
  call("/admin/api/" + path, { method, body, headers: { authorization: `Bearer ${E.ADMIN_TOKEN}` } });

/** A licence made by the owner: its id and the key (shown once). */
async function licence(extra: Json = {}): Promise<{ id: string; key: string }> {
  const r = await admin("issue", { email: "buyer@example.com", ...extra });
  expect(r.status).toBe(200);
  return { id: r.json.licence.id, key: r.json.key };
}

describe("the calls of the application", () => {
  it("activate returns a licence file signed for the device, in the format the Pro pack checks", async () => {
    const { id, key } = await licence();
    const r = await call("/v1/activate", { body: { key, device: device(1) } });
    expect(r.status).toBe(200);
    const p = await verifyFile(E.LICENCE_PUBLIC_KEY, r.json.licence);
    expect(p).toMatchObject({ v: 1, id, features: ["all"], major: 1, device: device(1), check_days: 14 });
    expect(Math.abs((p!.issued as number) - Date.now() / 1000)).toBeLessThan(30);
    // the key is accepted in any case and with spaces
    const again = await call("/v1/activate", { body: { key: ` ${key.toLowerCase()} `, device: device(1) } });
    expect(again.status).toBe(200);
  });

  it("names the key it signed with, and renews files of the older key (or without a key id) with the current one", async () => {
    const { key } = await licence();
    const a = await call("/v1/activate", { body: { key, device: device(1) } });
    expect(await verifyFile(E.LICENCE_PUBLIC_KEY, a.json.licence)).toMatchObject({ kid: "k1" });
    // a file from before the key ids: the same payload without `kid`, signed by the same key
    const { kid, ...old } = (await verifyFile(E.LICENCE_PUBLIC_KEY, a.json.licence))!;
    expect(kid).toBe("k1");
    const legacy = await signFile(E.LICENCE_SIGNING_KEY, old);
    const r = await call("/v1/refresh", { body: { licence: legacy, device: device(1) } });
    expect(r.status).toBe(200);
    expect(await verifyFile(E.LICENCE_PUBLIC_KEY, r.json.licence)).toMatchObject({ kid: "k1" });
    // a file that names a key this server does not trust is not accepted
    const stranger = await signFile(E.LICENCE_SIGNING_KEY, { ...old, kid: "k9" });
    expect((await call("/v1/refresh", { body: { licence: stranger, device: device(1) } })).json.error).toBe("bad_licence");
  });

  it("refuses an unknown key and a bad request", async () => {
    expect((await call("/v1/activate", { body: { key: "ASIDE-NOPE0-NOPE0-NOPE0-NOPE0", device: device(1) } })).json.error).toBe("invalid_key");
    expect((await call("/v1/activate", { body: { key: "x", device: "not a device" } })).json.error).toBe("bad_request");
    expect((await call("/v1/activate", { body: "not json" })).status).toBe(400);
  });

  it("allows two devices, refuses the third, and a freed place can be taken again", async () => {
    const { key } = await licence();
    const first = await call("/v1/activate", { body: { key, device: device(1) } });
    expect((await call("/v1/activate", { body: { key, device: device(2) } })).status).toBe(200);
    const third = await call("/v1/activate", { body: { key, device: device(3) } });
    expect(third.status).toBe(409);
    expect(third.json).toMatchObject({ error: "device_limit", devices: 2, max: 2 });
    // freeing the first device (with the file it got) makes room
    expect((await call("/v1/deactivate", { body: { licence: first.json.licence, device: device(1) } })).status).toBe(200);
    expect((await call("/v1/activate", { body: { key, device: device(3) } })).status).toBe(200);
  });

  it("refresh renews the file for an activated device only", async () => {
    const { key } = await licence();
    const first = await call("/v1/activate", { body: { key, device: device(1) } });
    const renewed = await call("/v1/refresh", { body: { licence: first.json.licence, device: device(1) } });
    expect(renewed.status).toBe(200);
    expect(await verifyFile(E.LICENCE_PUBLIC_KEY, renewed.json.licence)).toMatchObject({ device: device(1) });
    // another device with the same file, and a forged file
    expect((await call("/v1/refresh", { body: { licence: first.json.licence, device: device(2) } })).json.error).toBe("bad_licence");
    const forged = { payload: first.json.licence.payload, sig: first.json.licence.sig.replace(/^./, (c: string) => (c === "A" ? "B" : "A")) };
    expect((await call("/v1/refresh", { body: { licence: forged, device: device(1) } })).json.error).toBe("bad_licence");
  });

  it("a revoked licence cannot activate or refresh", async () => {
    const { id, key } = await licence();
    const first = await call("/v1/activate", { body: { key, device: device(1) } });
    expect((await admin(`licence/${id}/revoke`, {})).status).toBe(200);
    expect((await call("/v1/refresh", { body: { licence: first.json.licence, device: device(1) } })).json.error).toBe("revoked");
    expect((await call("/v1/activate", { body: { key, device: device(2) } })).json.error).toBe("revoked");
    expect((await admin(`licence/${id}/restore`, {})).status).toBe(200);
    expect((await call("/v1/refresh", { body: { licence: first.json.licence, device: device(1) } })).status).toBe(200);
  });

  it("the check interval and the features of a licence go into the file", async () => {
    const { id, key } = await licence({ check_days: 30, features: "replace.regex,mt.batch", max_devices: 1 });
    const r = await call("/v1/activate", { body: { key, device: device(1) } });
    expect(await verifyFile(E.LICENCE_PUBLIC_KEY, r.json.licence)).toMatchObject({ check_days: 30, features: ["replace.regex", "mt.batch"] });
    expect((await call("/v1/activate", { body: { key, device: device(2) } })).status).toBe(409);
    await admin(`licence/${id}/check_days`, { days: 14 });
    const again = await call("/v1/refresh", { body: { licence: r.json.licence, device: device(1) } });
    expect(await verifyFile(E.LICENCE_PUBLIC_KEY, again.json.licence)).toMatchObject({ check_days: 14 });
  });
});

describe("the payment provider", () => {
  const order = (event: string, id: number, extra: Json = {}) => ({ meta: { event_name: event }, data: { id, attributes: { user_email: "buyer@example.com", status: "paid", ...extra } } });
  async function send(payload: unknown, secret = E.WEBHOOK_SECRET) {
    const raw = JSON.stringify(payload);
    return call("/v1/webhook/lemonsqueezy", { body: raw, headers: { "x-signature": await hmacSha256Hex(secret, raw) } });
  }

  it("refuses a webhook with a wrong signature", async () => {
    expect((await send(order("order_created", 1), "wrong-secret")).status).toBe(401);
    expect((await call("/v1/webhook/lemonsqueezy", { body: order("order_created", 1) })).status).toBe(401);
  });

  it("a paid order makes one licence, a replay makes none, a refund revokes it", async () => {
    const first = await send(order("order_created", 4242));
    expect(first.status).toBe(200);
    const id = first.json.licence as string;
    expect((await send(order("order_created", 4242))).json.duplicate).toBe(true);
    const rows = await E.DB.prepare("SELECT COUNT(*) AS n FROM licences WHERE order_ref = 'ls-4242'").first<{ n: number }>();
    expect(rows?.n).toBe(1);
    expect((await send(order("order_refunded", 4242))).status).toBe(200);
    expect((await admin(`licence/${id}`)).json.licence.state).toBe("revoked");
    // events that do not matter are accepted and ignored
    expect((await send(order("subscription_created", 5))).json.ignored).toBe(true);
    expect((await send(order("order_created", 6, { status: "pending" }))).json.ignored).toBe(true);
  });
});

describe("the admin", () => {
  it("needs the token", async () => {
    expect((await call("/admin/api/overview", { method: "GET" })).status).toBe(401);
    expect((await call("/admin/api/overview", { method: "GET", headers: { authorization: "Bearer wrong" } })).status).toBe(401);
  });

  it("shows licences, devices and what happened", async () => {
    const { id, key } = await licence();
    await call("/v1/activate", { body: { key, device: device(1) } });
    const o = await admin("overview");
    expect(o.json.stats.active).toBeGreaterThanOrEqual(1);
    const row = o.json.licences.find((l: Json) => l.id === id);
    expect(row).toMatchObject({ email: "buyer@example.com", devices: 1, max_devices: 2 });
    expect(row.key_hash).toBeUndefined();
    const d = await admin(`licence/${id}`);
    expect(d.json.activations).toHaveLength(1);
    expect(d.json.events.map((e: Json) => e.kind)).toEqual(expect.arrayContaining(["issued", "activate"]));
    expect((await admin("overview?q=buyer@example")).json.licences.length).toBeGreaterThanOrEqual(1);
    // the owner frees a device
    await admin(`licence/${id}/release`, { device: device(1) });
    expect((await admin(`licence/${id}`)).json.activations[0].released).not.toBeNull();
  });

  it("a new key replaces the old one", async () => {
    const { id, key } = await licence();
    const r = await admin(`licence/${id}/rekey`, {});
    expect(r.json.key).toMatch(/^ASIDE-/);
    expect((await call("/v1/activate", { body: { key, device: device(1) } })).json.error).toBe("invalid_key");
    expect((await call("/v1/activate", { body: { key: r.json.key, device: device(1) } })).status).toBe(200);
  });

  it("serves the admin page", async () => {
    const r = await exports.default.fetch(new Request("https://licence.test/admin"));
    expect(r.status).toBe(200);
    expect(await r.text()).toContain("Aside – licences");
    expect(r.headers.get("content-security-policy")).toContain("frame-ancestors 'none'");
  });
});

describe("the e-mail with the key", () => {
  it("has a text and an HTML version with the key, a Reply-To, and escapes what it is given", async () => {
    const { keyEmail, mailKey } = await import("../src/mail.ts");
    const mail = keyEmail("ASIDE-AAAAA-BBBBB-CCCCC-DDDDD", "L-<TEST>");
    expect(mail.subject).toBe("Your Aside Pro licence key");
    expect(mail.text).toContain("ASIDE-AAAAA-BBBBB-CCCCC-DDDDD");
    expect(mail.html).toContain("ASIDE-AAAAA-BBBBB-CCCCC-DDDDD");
    expect(mail.html).toContain("L-&lt;TEST&gt;");
    expect(mail.html).not.toContain("L-<TEST>");

    let sent: Json | null = null;
    const original = globalThis.fetch;
    globalThis.fetch = (async (_url: unknown, init?: RequestInit) => {
      sent = JSON.parse(String(init?.body));
      return new Response("{}", { status: 200 });
    }) as typeof fetch;
    try {
      const withKey = { ...E, RESEND_API_KEY: "re_test", REPLY_TO: "owner@example.com", MAIL_FROM: "Aside <licence@example.com>" } as Env;
      expect(await mailKey(withKey, "buyer@example.com", "ASIDE-AAAAA-BBBBB-CCCCC-DDDDD", "L-1")).toBe(true);
      expect(sent).toMatchObject({ from: "Aside <licence@example.com>", to: "buyer@example.com", reply_to: "owner@example.com", subject: "Your Aside Pro licence key" });
      expect(String((sent as Json | null)?.html)).toContain("ASIDE-AAAAA-BBBBB-CCCCC-DDDDD");
      // nothing is sent without a Resend key or an address
      sent = null;
      expect(await mailKey({ ...E, RESEND_API_KEY: undefined } as Env, "buyer@example.com", "K", "L")).toBe(false);
      expect(await mailKey(withKey, "", "K", "L")).toBe(false);
      expect(sent).toBeNull();
    } finally {
      globalThis.fetch = original;
    }
  });
});

describe("the admin tools", () => {
  it("changes the number of devices of one licence", async () => {
    const { id, key } = await licence({ max_devices: 1 });
    await call("/v1/activate", { body: { key, device: device(1) } });
    expect((await call("/v1/activate", { body: { key, device: device(2) } })).status).toBe(409);
    expect((await admin(`licence/${id}/max_devices`, { n: 3 })).status).toBe(200);
    expect((await call("/v1/activate", { body: { key, device: device(2) } })).status).toBe(200);
    expect((await admin(`licence/${id}/max_devices`, { n: 0 })).status).toBe(400);
    expect((await admin(`licence/${id}/max_devices`, { n: 99 })).status).toBe(400);
  });

  it("edits the e-mail and the note, and erases the personal data (also in the log)", async () => {
    const { id } = await licence({ email: "first@example.com", note: "n1" });
    expect((await admin(`licence/${id}/update`, { email: "second@example.com", note: "n2" })).status).toBe(200);
    expect((await admin(`licence/${id}`)).json.licence).toMatchObject({ email: "second@example.com", note: "n2" });
    expect((await admin(`licence/${id}/update`, { email: "not an address", note: "" })).status).toBe(400);
    await E.DB.prepare("INSERT INTO events (at, licence_id, kind, detail) VALUES (1, ?, 'key-mailed', 'second@example.com')").bind(id).run();
    expect((await admin(`licence/${id}/erase`, {})).status).toBe(200);
    const d = await admin(`licence/${id}`);
    expect(d.json.licence).toMatchObject({ email: "", note: "" });
    expect(JSON.stringify(d.json.events)).not.toContain("example.com");
    expect(d.json.events.map((e: Json) => e.kind)).toContain("erased");
  });

  it("issues many licences at once, each with a key that works", async () => {
    const r = await admin("issue_bulk", { count: 3, note: "testers", max_devices: 1, check_days: 30 });
    expect(r.status).toBe(200);
    expect(r.json.licences).toHaveLength(3);
    expect(new Set(r.json.licences.map((l: Json) => l.key)).size).toBe(3);
    const first = r.json.licences[0];
    const act = await call("/v1/activate", { body: { key: first.key, device: device(1) } });
    expect(await verifyFile(E.LICENCE_PUBLIC_KEY, act.json.licence)).toMatchObject({ id: first.id, check_days: 30 });
    expect((await admin("issue_bulk", { count: 0 })).status).toBe(400);
    expect((await admin("issue_bulk", { count: 101 })).status).toBe(400);
  });

  it("exports the licences as CSV without keys and without spreadsheet formulas", async () => {
    await licence({ email: "=cmd|calc@example.com", note: 'has "quotes", and a comma' });
    const r = await exports.default.fetch(new Request("https://licence.test/admin/api/export", { headers: { authorization: `Bearer ${E.ADMIN_TOKEN}` } }));
    expect(r.status).toBe(200);
    expect(r.headers.get("content-type")).toContain("text/csv");
    const text = await r.text();
    expect(text.split("\r\n")[0]).toBe("id,email,major,features,check_days,max_devices,state,created,devices,last_seen,order_ref,note");
    expect(text).toContain("\"'=cmd|calc@example.com\"");
    expect(text).toContain('"has ""quotes"", and a comma"');
    expect(text).not.toContain("ASIDE-");
    expect((await call("/admin/api/export", { method: "GET" })).status).toBe(401);
  });

  it("can show only the refused attempts", async () => {
    await call("/v1/activate", { body: { key: "ASIDE-ZZZZZ-ZZZZZ-ZZZZZ-ZZZZZ", device: device(1) } });
    const r = await admin("overview?events=refused");
    expect(r.json.events.length).toBeGreaterThan(0);
    expect(r.json.events.every((e: Json) => e.kind === "refused")).toBe(true);
    // a bad kind falls back to the recent activity
    expect((await admin("overview?events=%27%3B--")).status).toBe(200);
  });
});

describe("devices of a licence", () => {
  it("keeps the name and the version of a computer and lists them for the licence holder", async () => {
    const { key } = await licence();
    const a = await call("/v1/activate", { body: { key, device: device(1), name: "  Notebook\u0001 ", version: "0.3.1" } });
    await call("/v1/activate", { body: { key, device: device(2), name: "Home PC" } });
    const list = await call("/v1/devices", { body: { licence: a.json.licence, device: device(1) } });
    expect(list.status).toBe(200);
    expect(list.json).toMatchObject({ max: 2, releases_left: 2, release_limit: 2 });
    const byId = Object.fromEntries(list.json.devices.map((d: Json) => [d.id, d]));
    expect(byId[device(1)]).toMatchObject({ name: "Notebook", version: "0.3.1", current: true });
    expect(byId[device(2)]).toMatchObject({ name: "Home PC", current: false });
    // a refresh without a name keeps it, a refresh with one changes it
    await call("/v1/refresh", { body: { licence: a.json.licence, device: device(1), version: "0.4.0" } });
    await call("/v1/refresh", { body: { licence: a.json.licence, device: device(1), name: "Work laptop" } });
    const again = await call("/v1/devices", { body: { licence: a.json.licence, device: device(1) } });
    expect(again.json.devices.find((d: Json) => d.id === device(1))).toMatchObject({ name: "Work laptop", version: "0.4.0" });
    // a forged file sees nothing
    const forged = { payload: a.json.licence.payload, sig: "AAAA" };
    expect((await call("/v1/devices", { body: { licence: forged, device: device(1) } })).status).toBe(403);
  });

  it("lets the user free another computer twice for good, and the owner without a limit", async () => {
    const { id, key } = await licence({ max_devices: 3 });
    const first = await call("/v1/activate", { body: { key, device: device(1) } });
    for (const n of [2, 3]) await call("/v1/activate", { body: { key, device: device(n) } });
    const free = (target: string) => call("/v1/release", { body: { licence: first.json.licence, device: device(1), target } });
    expect((await free(device(2))).json).toMatchObject({ ok: true, releases_left: 1 });
    expect((await free(device(3))).json).toMatchObject({ ok: true, releases_left: 0 });
    // nothing is left of the two places, and a third release is refused (for good: it does not come back with time)
    await call("/v1/activate", { body: { key, device: device(4) } });
    const refused = await free(device(4));
    expect(refused.status).toBe(429);
    expect(refused.json).toMatchObject({ error: "release_limit", limit: 2 });
    // deactivating counts too
    expect((await call("/v1/deactivate", { body: { licence: first.json.licence, device: device(1) } })).status).toBe(429);
    // a place that is not held, and a bad request
    expect((await free(device(9))).status).toBe(404); // not held: reported before the limit is looked at
    // the owner frees a computer without any limit
    expect((await admin(`licence/${id}/release`, { device: device(4) })).status).toBe(200);
    const list = await call("/v1/devices", { body: { licence: first.json.licence, device: device(1) } });
    expect(list.json.devices.map((d: Json) => d.id)).toEqual([device(1)]);
    expect(list.json.releases_left).toBe(0);
  });

  it("refuses to free a place that is not held and a malformed target", async () => {
    const { key } = await licence();
    const first = await call("/v1/activate", { body: { key, device: device(1) } });
    expect((await call("/v1/release", { body: { licence: first.json.licence, device: device(1), target: device(7) } })).status).toBe(404);
    expect((await call("/v1/release", { body: { licence: first.json.licence, device: device(1), target: "nope" } })).status).toBe(400);
  });
});

describe("a lost key", () => {
  const request = (email: string) => call("/v1/recover/request", { body: { email } });
  const confirm = (email: string, code: string) => call("/v1/recover/confirm", { body: { email, code } });
  /** The code is only mailed, so the tests put a known one in the database (as the hash the server keeps). */
  async function setCode(id: string, code: string, expires = Math.floor(Date.now() / 1000) + 600, attempts = 0) {
    await E.DB.prepare("INSERT OR REPLACE INTO recovery_codes (licence_id, code_hash, expires, attempts) VALUES (?,?,?,?)")
      .bind(id, await sha256Hex(`aside-code:${id}:${code.replace(/[^A-Z0-9]/gi, "").toUpperCase()}`), expires, attempts)
      .run();
  }

  it("answers the same for every address and keeps a code only for a real licence", async () => {
    const { id } = await licence({ email: "owner@example.com" });
    expect(await request("owner@example.com")).toMatchObject({ status: 200, json: { ok: true } });
    expect(await request("stranger@example.com")).toMatchObject({ status: 200, json: { ok: true } });
    expect((await E.DB.prepare("SELECT COUNT(*) AS n FROM recovery_codes").first<{ n: number }>())?.n).toBe(1);
    expect((await E.DB.prepare("SELECT licence_id FROM recovery_codes").first<{ licence_id: string }>())?.licence_id).toBe(id);
    expect((await call("/v1/recover/request", { body: { email: "not an address" } })).status).toBe(400);
  });

  it("asks for a code twice only after an hour, and three times a day", async () => {
    await licence({ email: "limit@example.com" });
    expect((await request("limit@example.com")).status).toBe(200);
    const again = await request("limit@example.com");
    expect(again.status).toBe(429);
    expect(again.json).toMatchObject({ error: "too_many" });
    // the limit counts requests for any address, so it tells nothing about who is a customer
    expect((await request("nobody@example.com")).status).toBe(200);
    expect((await request("nobody@example.com")).status).toBe(429);
    // three a day: move the earlier ones back by more than an hour
    const h = await sha256Hex("aside-recover:limit@example.com");
    const t0 = Math.floor(Date.now() / 1000);
    await E.DB.prepare("UPDATE recovery_requests SET at = ? WHERE email_hash = ?").bind(t0 - 7200, h).run();
    expect((await request("limit@example.com")).status).toBe(200);
    await E.DB.prepare("UPDATE recovery_requests SET at = ? WHERE email_hash = ? AND at > ?").bind(t0 - 3700, h, t0 - 10).run();
    expect((await request("limit@example.com")).status).toBe(200);
    await E.DB.prepare("UPDATE recovery_requests SET at = ? WHERE email_hash = ? AND at > ?").bind(t0 - 3600 - 100, h, t0 - 10).run();
    expect((await request("limit@example.com")).status).toBe(429);
  });

  it("replaces the key only with the right code, and the old key stops working while the computers keep working", async () => {
    const { id, key } = await licence({ email: "buyer2@example.com" });
    const act = await call("/v1/activate", { body: { key, device: device(1) } });
    await request("buyer2@example.com");
    await setCode(id, "ABCD-2345");
    expect((await confirm("buyer2@example.com", "WRONG-000")).json).toMatchObject({ error: "invalid_code" });
    // the old key still works until the right code is entered
    expect((await call("/v1/activate", { body: { key, device: device(1) } })).status).toBe(200);
    const done = await confirm("buyer2@example.com", "abcd 2345");
    expect(done.status).toBe(200);
    expect(done.json.key).toMatch(/^ASIDE-/);
    expect(done.json.key).not.toBe(key);
    expect((await call("/v1/activate", { body: { key, device: device(2) } })).json.error).toBe("invalid_key");
    expect((await call("/v1/activate", { body: { key: done.json.key, device: device(2) } })).status).toBe(200);
    // the licence file of the first computer is still good
    expect((await call("/v1/refresh", { body: { licence: act.json.licence, device: device(1) } })).status).toBe(200);
    // a code works once
    expect((await confirm("buyer2@example.com", "ABCD-2345")).json.error).toBe("invalid_code");
  });

  it("allows five attempts and a code of fifteen minutes, and a recovery only once per licence", async () => {
    const { id } = await licence({ email: "buyer3@example.com" });
    await setCode(id, "AAAA-2222", Math.floor(Date.now() / 1000) - 5); // expired
    expect((await confirm("buyer3@example.com", "AAAA-2222")).json.error).toBe("invalid_code");
    await setCode(id, "AAAA-2222");
    for (let i = 0; i < 5; i++) expect((await confirm("buyer3@example.com", "BBBB-3333")).json.error).toBe("invalid_code");
    expect((await confirm("buyer3@example.com", "AAAA-2222")).json.error).toBe("invalid_code"); // locked after five wrong tries
    await setCode(id, "CCCC-4444");
    expect((await confirm("buyer3@example.com", "CCCC-4444")).status).toBe(200);
    // the second recovery is refused: no new code, and a confirm with a planted one is refused too
    expect((await request("buyer3@example.com")).status).toBe(200);
    expect((await E.DB.prepare("SELECT COUNT(*) AS n FROM recovery_codes WHERE licence_id = ?").bind(id).first<{ n: number }>())?.n).toBe(0);
    await setCode(id, "DDDD-5555");
    expect((await confirm("buyer3@example.com", "DDDD-5555")).json.error).toBe("already_used");
  });

  it("does nothing for a cancelled licence", async () => {
    const { id } = await licence({ email: "gone@example.com" });
    await admin(`licence/${id}/revoke`, {});
    await request("gone@example.com");
    expect((await E.DB.prepare("SELECT COUNT(*) AS n FROM recovery_codes WHERE licence_id = ?").bind(id).first<{ n: number }>())?.n).toBe(0);
  });
});
