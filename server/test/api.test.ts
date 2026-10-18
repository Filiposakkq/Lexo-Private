import { env, exports } from "cloudflare:workers";
import { describe, expect, it } from "vitest";
import { hmacSha256Hex, verifyFile } from "../src/crypto.ts";
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
