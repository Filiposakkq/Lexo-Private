import { env, exports } from "cloudflare:workers";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { daily } from "../src/cron.ts";
import type { Env } from "../src/env.ts";
import { render } from "../src/mail.ts";

type Json = Record<string, any>;
const E = env as unknown as Env;
const DAY = 86400;

async function call(path: string, init: { method?: string; body?: unknown; headers?: Record<string, string> } = {}): Promise<{ status: number; json: Json; res: Response }> {
  const res = await exports.default.fetch(
    new Request("https://aside.test" + path, {
      method: init.method ?? (init.body === undefined ? "GET" : "POST"),
      // every call comes from another address unless a test says otherwise (the storage is shared by the tests of this file)
      headers: { "content-type": "application/json", "cf-connecting-ip": "198.51.100." + Math.floor(Math.random() * 250), ...init.headers },
      body: init.body === undefined ? undefined : typeof init.body === "string" ? init.body : JSON.stringify(init.body),
      redirect: "manual",
    }),
  );
  const json = (await res.clone().json().catch(() => ({}))) as Json;
  return { status: res.status, json, res };
}
const admin = (path: string, body?: unknown) => call("/admin/api/" + path, { body, headers: { authorization: `Bearer ${E.ADMIN_TOKEN}` } });

// every application comes from another e-mail unless a test says otherwise
const uid = () => Math.random().toString(36).slice(2, 10);
const tester = (extra: Json = {}) => ({
  route: "test",
  fields: {
    nick: "Mira", email: `mira${uid()}@example.com`, speak: ["English"], win: "Windows 11", ram: "16 GB", exp: "I translate regularly",
    games: "Eternum, Being a DIK", bug: "When I pressed save twice quickly the app showed an error and the line was not saved, I expected it to save once.",
    hours: "3–5 h a week", agree: ["a", "b", "c", "d"], ...extra,
  },
  answers: [["Nickname", "Mira"], ["Games", "Eternum"]],
});
const licensee = (extra: Json = {}) => ({
  route: "licence",
  fields: {
    nick: "Tom", email: `tom${uid()}@example.com`, lang: "Czech", game: "Eternum 0.9", link: "https://example.com/thread", progress: "25–75 %", size: "50k–150k",
    team: "Just me", tools: "Notepad++", why: "Proofreading seventy thousand lines by hand takes me months, I want the checks.", agree: ["a", "b", "c", "d"], ...extra,
  },
  answers: [["Game and version", "Eternum 0.9"]],
});

beforeAll(async () => {
  // many applications are made in this file within one "day"
  await admin("settings", { apply_day: 500 });
});

describe("beta applications", () => {
  it("accepts a complete application and counts places only for accepted ones", async () => {
    const r = await call("/api/apply", { body: tester() });
    expect(r.status).toBe(200);
    expect(r.json).toMatchObject({ ok: true, waitlist: false });
    const cfg = await call("/api/config");
    expect(cfg.json.program).toMatchObject({ round: "Round 1", open: true, test: { places: 4, taken: 0 }, licence: { places: 3, taken: 0 } });
    expect((await admin(`application/${r.json.id}/decide`, { state: "accepted" })).json.code).toMatch(/^[a-z2-9]{24}$/);
    expect((await call("/api/config")).json.program.test.taken).toBe(1);
  });

  it("refuses missing or too short answers, with the name of the field", async () => {
    expect((await call("/api/apply", { body: tester({ bug: "it broke" }) })).json).toMatchObject({ error: "missing", field: "bug" });
    expect((await call("/api/apply", { body: tester({ email: "nope" }) })).json.field).toBe("email");
    expect((await call("/api/apply", { body: tester({ agree: ["a"] }) })).json.field).toBe("agree");
    expect((await call("/api/apply", { body: licensee({ link: "" }) })).json.field).toBe("link");
    expect((await call("/api/apply", { body: { route: "other", fields: {} } })).status).toBe(400);
  });

  it("an exception replaces the public link with a reason", async () => {
    expect((await call("/api/apply", { body: licensee({ link: "", exception: true }) })).json.field).toBe("why_ex");
    const ok = await call("/api/apply", { body: licensee({ link: "", exception: true, why_ex: "A team project that goes public next month with the first episode, 40 % done." }) });
    expect(ok.status).toBe(200);
  });

  it("closed rounds refuse, a full round puts people on the waiting list", async () => {
    await admin("settings", { places_licence: 0 });
    expect((await call("/api/apply", { body: licensee() })).json.waitlist).toBe(true);
    await admin("settings", { open: false });
    expect((await call("/api/apply", { body: licensee() })).json.error).toBe("closed");
    await admin("settings", { open: true, places_licence: 3 });
  });

  it("the hidden field silently drops bots, and one address may apply at most twice a day", async () => {
    const bot = await call("/api/apply", { body: { ...tester({ nick: "Robot" }), website: "spam" } });
    expect(bot.json.ok).toBe(true);
    expect(((await admin("applications?state=")).json.items as Json[]).some((a) => a.nick === "Robot")).toBe(false);
    const same = { "cf-connecting-ip": "203.0.113.9" };
    for (let i = 0; i < 2; i++) expect((await call("/api/apply", { body: tester(), headers: same })).status).toBe(200);
    expect((await call("/api/apply", { body: tester(), headers: same })).json.error).toBe("too_many");
  });

  it("one open application per e-mail and route; after a no the person may apply again", async () => {
    const email = `twin${uid()}@example.com`;
    const first = await call("/api/apply", { body: tester({ email }) });
    expect(first.status).toBe(200);
    const again = await call("/api/apply", { body: tester({ email: email.toUpperCase() }) });
    expect(again.status).toBe(409);
    expect(again.json.error).toBe("already");
    expect((await call("/api/apply", { body: licensee({ email }) })).status).toBe(200);
    await admin(`application/${first.json.id}/decide`, { state: "declined" });
    expect((await call("/api/apply", { body: tester({ email }) })).status).toBe(200);
  });
});

describe("confirming the e-mail", () => {
  it("with e-mail set up, an application counts only after the link in the confirmation e-mail", async () => {
    const sent: Json[] = [];
    const real = globalThis.fetch;
    const spy = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
      const u = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
      if (u.startsWith("https://api.resend.com/")) {
        sent.push(JSON.parse(String(init?.body)));
        return new Response('{"id":"x"}', { status: 200 });
      }
      return real(input, init);
    });
    (E as { RESEND_API_KEY?: string }).RESEND_API_KEY = "re_test";
    try {
      const email = `confirm${uid()}@example.com`;
      const r = await call("/api/apply", { body: tester({ email }) });
      expect(r.json).toMatchObject({ ok: true, confirm: true, mailed: true });
      expect(sent.at(-1)!.to).toBe(email);
      const token = /#confirm=([a-z2-9]+)/.exec(sent.at(-1)!.text)![1]!;
      // not in the list yet, and the owner was not told
      expect(((await admin("applications?state=")).json.items as Json[]).some((a) => a.id === r.json.id)).toBe(false);
      expect(((await admin("applications?state=unconfirmed")).json.items as Json[]).some((a) => a.id === r.json.id)).toBe(true);
      expect(sent).toHaveLength(1);
      // the same e-mail cannot be used again while it waits
      expect((await call("/api/apply", { body: tester({ email }) })).json.error).toBe("already");
      expect((await call("/api/confirm", { body: { token: "abcdefghjkmnpqrstuvwxyz2" } })).status).toBe(404);
      const ok = await call("/api/confirm", { body: { token } });
      expect(ok.json).toMatchObject({ ok: true, nick: "Mira" });
      expect(sent.at(-1)!.subject).toContain("New application");
      expect((await call("/api/confirm", { body: { token } })).status).toBe(404);
      expect(((await admin("applications?state=new")).json.items as Json[]).some((a) => a.id === r.json.id)).toBe(true);
      // the history of e-mails of the person has the confirmation e-mail
      expect(((await admin(`application/${r.json.id}`)).json.mails as Json[]).map((m) => m.kind)).toContain("verify");
    } finally {
      delete (E as { RESEND_API_KEY?: string }).RESEND_API_KEY;
      spy.mockRestore();
    }
  });

  it("unconfirmed applications disappear after 48 hours", async () => {
    const now = Math.floor(Date.now() / 1000);
    await E.DB.prepare("INSERT INTO applications (id, route, created, nick, email, answers, verify_token) VALUES ('oldunconf', 'test', ?, 'Old', 'old@example.com', '[]', 'oldtokenoldtokenoldtoken')").bind(now - 3 * DAY).run();
    await daily(E, now);
    expect((await admin("application/oldunconf")).status).toBe(404);
  });
});

describe("people", () => {
  it("an accepted person can only be removed (not declined), which kills the link; readmitting gives a new link", async () => {
    const id = (await call("/api/apply", { body: tester() })).json.id;
    const code = (await admin(`application/${id}/decide`, { state: "accepted" })).json.code;
    expect((await call(`/api/tester/${code}`)).status).toBe(200);
    expect((await admin(`application/${id}/decide`, { state: "declined" })).json.error).toBe("use_remove");
    expect(((await admin("people")).json.items as Json[]).find((p) => p.id === id)).toMatchObject({ state: "accepted", has_link: 1 });
    await admin(`application/${id}/remove`, { reason: "Never tested" });
    expect((await call(`/api/tester/${code}`)).status).toBe(404);
    expect((await call("/api/config")).json.program.test.taken).toBe(((await admin("people?route=test")).json.items as Json[]).filter((p) => p.state === "accepted").length);
    const again = (await admin(`application/${id}/readmit`, {})).json.code;
    expect(again).not.toBe(code);
    expect((await call(`/api/tester/${again}`)).status).toBe(200);
    const fresh = (await admin(`application/${id}/newcode`, {})).json.code;
    expect((await call(`/api/tester/${again}`)).status).toBe(404);
    expect((await call(`/api/tester/${fresh}`)).status).toBe(200);
    expect(((await admin(`application/${id}`)).json.log as Json[]).map((l) => l.action)).toEqual(expect.arrayContaining(["remove", "readmit", "new link"]));
  });

  it("wrong links are limited per address, and the pages can be switched off", async () => {
    const ip = { "cf-connecting-ip": "203.0.113.77" };
    for (let i = 0; i < 20; i++) await call("/api/tester/abcdefghjkmnpqrstuvw" + i, { headers: ip });
    expect((await call("/api/tester/abcdefghjkmnpqrstuvwx", { headers: ip })).status).toBe(429);
    const id = (await call("/api/apply", { body: tester() })).json.id;
    const code = (await admin(`application/${id}/decide`, { state: "accepted" })).json.code;
    await admin("settings", { pages_open: false });
    expect((await call(`/api/tester/${code}`)).json.error).toBe("paused");
    await admin("settings", { pages_open: true });
    expect((await call(`/api/tester/${code}`)).status).toBe(200);
  });
});

describe("feedback of sponsored licences", () => {
  it("is asked after 14 days, reminded after 4 more, then the 4-week form; both answers make the licence ready", async () => {
    const id = (await call("/api/apply", { body: licensee() })).json.id;
    const code = (await admin(`application/${id}/decide`, { state: "accepted" })).json.code;
    const app = (await admin(`application/${id}`)).json.item;
    expect(app.trial_start).toBeGreaterThan(0);
    const t0 = app.trial_start as number;

    expect((await daily(E, t0 + 13 * DAY)).sent).toBe(0);
    expect((await daily(E, t0 + 14 * DAY)).sent).toBe(1);
    expect((await daily(E, t0 + 15 * DAY)).sent).toBe(0);
    expect((await daily(E, t0 + 18 * DAY)).sent).toBe(1); // the reminder
    expect((await daily(E, t0 + 19 * DAY)).sent).toBe(0);

    const info = await call(`/api/feedback/${code}`);
    expect(info.json).toMatchObject({ nick: "Tom", game: "Eternum 0.9", next: "2w" });
    const answers = [["Used", "Daily"], ["Good", "Checks"], ["Bad", "Nothing"]];
    expect((await call(`/api/feedback/${code}`, { body: { which: "4w", answers } })).status).toBe(400);
    expect((await call(`/api/feedback/${code}`, { body: { which: "2w", answers } })).status).toBe(200);
    expect((await call(`/api/feedback/${code}`, { body: { which: "2w", answers } })).json.error).toBe("already_sent");

    expect((await daily(E, t0 + 28 * DAY)).sent).toBe(1);
    expect((await call(`/api/feedback/${code}`, { body: { which: "4w", answers } })).status).toBe(200);
    expect((await admin("overview")).json.counts.ready).toBe(1);
    expect((await daily(E, t0 + 40 * DAY)).sent).toBe(0);
  });

  it("an unknown or not accepted code gets nothing", async () => {
    expect((await call("/api/feedback/abcdefghjkmnpqrstuvw")).status).toBe(404);
    expect((await call("/api/feedback/x")).status).toBe(404);
  });
});

describe("bug reports of testers", () => {
  it("only an accepted tester can report, with an optional screenshot that only the admin can see", async () => {
    const id = (await call("/api/apply", { body: tester() })).json.id;
    const bug = { title: "Save fails twice", steps: "Open a project, edit a line, press Ctrl+Enter twice fast.", severity: "broken", image: "data:image/png;base64,iVBORw0KGgo=" };
    expect((await call("/api/bug", { body: { ...bug, code: "abcdefghjkmnpqrstuvw" } })).status).toBe(404);
    const code = (await admin(`application/${id}/decide`, { state: "accepted" })).json.code;
    const r = await call("/api/bug", { body: { ...bug, code } });
    expect(r.status).toBe(200);
    expect((await call(`/api/tester/${code}`)).json.bugs[0]).toMatchObject({ title: "Save fails twice", state: "new" });
    const img = await call(`/admin/api/bug/${r.json.id}/image`, { headers: { authorization: `Bearer ${E.ADMIN_TOKEN}` } });
    expect(img.res.headers.get("content-type")).toBe("image/png");
    expect(new Uint8Array(await img.res.arrayBuffer()).slice(0, 4)).toEqual(new Uint8Array([0x89, 0x50, 0x4e, 0x47]));
    expect((await call(`/admin/api/bug/${r.json.id}/image`)).status).toBe(401);
    expect((await call("/api/bug", { body: { ...bug, code, title: "x" } })).json.field).toBe("title");
    await admin(`bug/${r.json.id}`, { state: "fixed", note: "in 2.0.1", reply: "Fixed in 2.0.1, thanks!" });
    const mine = (await call(`/api/tester/${code}`)).json.bugs[0];
    expect(mine).toMatchObject({ state: "fixed", reply: "Fixed in 2.0.1, thanks!" });
    expect(mine.note).toBeUndefined();
    const listed = ((await admin("bugs?state=fixed")).json.items as Json[]).find((b) => b.id === r.json.id)!;
    expect(listed.closed).toBeGreaterThan(0);
    expect(listed.image_size).toBeGreaterThan(0);
    await admin(`bug/${r.json.id}/delete-image`, {});
    expect((await call(`/admin/api/bug/${r.json.id}/image`, { headers: { authorization: `Bearer ${E.ADMIN_TOKEN}` } })).status).toBe(404);
    await admin(`bug/${r.json.id}/delete`, {});
    expect((await call(`/api/tester/${code}`)).json.bugs).toHaveLength(0);
  });
});

describe("roadmap votes", () => {
  const voter = "0123456789abcdef0123456789abcdef";
  it("counts one vote per browser, can be taken back, adds the votes given by hand, and refuses the Now column", async () => {
    const items = (await call("/api/roadmap")).json.items as Json[];
    const item = items.find((i) => i.col === "later")!;
    expect((await call("/api/vote", { body: { voter, item: item.id } })).json.votes).toBe(1);
    expect((await call("/api/vote", { body: { voter, item: item.id } })).json.votes).toBe(1);
    expect((await call(`/api/roadmap?voter=${voter}`)).json.items.find((i: Json) => i.id === item.id).mine).toBe(true);
    await admin(`roadmap/${item.id}`, { ...item, extra: 5 });
    expect((await call("/api/roadmap")).json.items.find((i: Json) => i.id === item.id).votes).toBe(6);
    expect((await call("/api/vote", { body: { voter, item: item.id, on: false } })).json.votes).toBe(5);
    const now = items.find((i) => i.col === "now")!;
    expect((await call("/api/vote", { body: { voter, item: now.id } })).status).toBe(404);
    expect((await call("/api/vote", { body: { voter: "short", item: item.id } })).status).toBe(400);
  });

  it("the admin adds, moves and deletes cards", async () => {
    const id = (await admin("roadmap", { title: "Dark mode for the app", col: "ideas", tier: "free" })).json.id;
    let ideas = ((await call("/api/roadmap")).json.items as Json[]).filter((i) => i.col === "ideas");
    expect(ideas.at(-1)!.id).toBe(id);
    await admin(`roadmap/${id}/move`, { dir: -1 });
    ideas = ((await call("/api/roadmap")).json.items as Json[]).filter((i) => i.col === "ideas");
    expect(ideas.at(-2)!.id).toBe(id);
    await admin(`roadmap/${id}/delete`, {});
    expect(((await call("/api/roadmap")).json.items as Json[]).some((i) => i.id === id)).toBe(false);
  });
});

describe("showcase, contact and releases", () => {
  it("a showcase entry appears on the website only after approval", async () => {
    expect(((await call("/api/showcase")).json.items as Json[]).map((i) => i.game)).toEqual(["Being a DIK"]);
    const entry = { game: "Eternum", lang: "Polish", link: "https://example.com/pl", translator: "Ola", email: "ola@example.com" };
    expect((await call("/api/showcase", { body: { ...entry, link: "nope" } })).json.field).toBe("link");
    expect((await call("/api/showcase", { body: entry })).status).toBe(200);
    expect((await call("/api/showcase")).json.items).toHaveLength(1);
    const pending = ((await admin("showcase")).json.items as Json[]).find((i) => i.state === "pending")!;
    await admin(`showcase/${pending.id}`, { ...pending, state: "approved" });
    expect((await call("/api/showcase")).json.items).toHaveLength(2);
  });

  it("a message is stored, listed for the admin and can be archived", async () => {
    expect((await call("/api/contact", { body: { name: "Ana", email: "ana@example.com", message: "short" } })).json.field).toBe("message");
    const r = await call("/api/contact", { body: { name: "Ana", email: "ana@example.com", topic: "Question", message: "Does Aside work with Ren'Py 6?" } });
    expect(r.status).toBe(200);
    expect((await admin("overview")).json.counts.messages).toBe(1);
    // without a mail key a reply is not sent and the message stays new
    expect((await admin(`message/${r.json.id}/reply`, { text: "Not yet, sorry." })).json.sent).toBe(false);
    await admin(`message/${r.json.id}`, { state: "archived" });
    expect((await admin("messages?state=archived")).json.items).toHaveLength(1);
  });

  it("only published releases are listed, and downloads are counted and redirected", async () => {
    await admin("release", { version: "2.0.0", date: "2026-11-01", notes: "new: the first public build", url: "https://example.com/Aside_2.0.0_x64-setup.exe", published: true });
    await admin("release", { version: "2.1.0", date: "2026-12-01", notes: "draft" });
    expect((await admin("release", { version: "3", url: "http://insecure.example.com" })).json.field).toBe("url");
    const list = (await call("/api/releases")).json.items as Json[];
    expect(list.map((r) => r.version)).toEqual(["2.0.0"]);
    const d = await call("/api/download/latest");
    expect(d.status).toBe(302);
    expect(d.res.headers.get("location")).toBe("https://example.com/Aside_2.0.0_x64-setup.exe");
    expect(((await admin("releases")).json.items as Json[]).find((r) => r.version === "2.0.0")!.downloads).toBe(1);
  });
});

describe("the admin", () => {
  it("needs the token", async () => {
    expect((await call("/admin/api/overview")).status).toBe(401);
    expect((await call("/admin/api/overview", { headers: { authorization: "Bearer wrong-token-0123456789" } })).status).toBe(401);
    expect((await admin("overview")).status).toBe(200);
    const page = await call("/admin");
    expect(page.res.headers.get("content-type")).toContain("text/html");
    expect(await page.res.text()).toContain("Aside admin");
  });

  it("deleting an application removes its feedback and bug reports", async () => {
    const id = (await call("/api/apply", { body: tester() })).json.id;
    const code = (await admin(`application/${id}/decide`, { state: "accepted" })).json.code;
    await call("/api/bug", { body: { code, title: "Crash on start", steps: "Start the app with a broken project folder." } });
    await admin(`application/${id}/delete`, {});
    expect((await admin(`application/${id}`)).status).toBe(404);
    expect(((await admin("bugs")).json.items as Json[]).some((b) => b.title === "Crash on start")).toBe(false);
  });
});

describe("data tools", () => {
  it("bulk delete, clean-up, search, export and the log", async () => {
    const a = (await call("/api/apply", { body: tester({ nick: "Searchable Sam" }) })).json.id;
    const b = (await call("/api/apply", { body: tester() })).json.id;
    await admin(`application/${b}/decide`, { state: "declined" });
    expect(((await admin("search?q=Searchable")).json.items as Json[]).some((x) => x.id === a)).toBe(true);
    const csv = await call("/admin/api/export/applications", { headers: { authorization: `Bearer ${E.ADMIN_TOKEN}` } });
    expect(csv.res.headers.get("content-type")).toContain("text/csv");
    expect(await csv.res.text()).toContain("Searchable Sam");
    const backup = await call("/admin/api/export/backup", { headers: { authorization: `Bearer ${E.ADMIN_TOKEN}` } });
    expect(Object.keys(backup.json)).toEqual(expect.arrayContaining(["applications", "bugs", "mails", "log"]));
    expect((await admin("cleanup", { what: "declined", days: 0 })).json.deleted).toBeGreaterThan(0);
    expect((await admin(`application/${b}`)).status).toBe(404);
    expect((await admin("bulk", { kind: "applications", ids: [a] })).json.deleted).toBe(1);
    expect((await admin(`application/${a}`)).status).toBe(404);
    const st = (await admin("storage")).json;
    expect(st.rows.applications).toBeGreaterThan(0);
    expect(((await admin("log")).json.items as Json[]).map((l) => l.action)).toEqual(expect.arrayContaining(["bulk delete", "clean up"]));
  });

  it("group e-mails need e-mail set up; templates are kept", async () => {
    expect((await admin("mail/send", { to: "test", subject: "Hi", body: "Hello {nick}" })).json.error).toBe("mail_not_configured");
    expect(((await admin("templates")).json.items as Json[]).map((t) => t.id)).toEqual(expect.arrayContaining(["build", "key"]));
    await admin("template", { name: "Thanks", subject: "Thank you", body: "Thanks {nick}" });
    expect(((await admin("templates")).json.items as Json[]).some((t) => t.name === "Thanks")).toBe(true);
  });
});

/** Pretends Resend and ntfy answer; records what was sent. */
function fakeOutside() {
  const sent: { url: string; body: any; headers: Record<string, string> }[] = [];
  const real = globalThis.fetch;
  const spy = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const u = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
    if (u.startsWith("https://api.resend.com/") || u.startsWith("https://ntfy.sh/")) {
      const raw = String(init?.body ?? "");
      let parsed: unknown = raw;
      try { parsed = JSON.parse(raw); } catch {}
      sent.push({ url: u, body: parsed, headers: Object.fromEntries(new Headers(init?.headers).entries()) });
      if (u === "https://api.resend.com/domains") return new Response('{"data":[{"name":"filiposakkq.dev","status":"verified"}]}');
      return new Response('{"id":"x"}', { status: 200 });
    }
    return real(input, init);
  });
  const e = E as unknown as Record<string, string | undefined>;
  e.RESEND_API_KEY = "re_test";
  e.NTFY_TOPIC = "aside-test-topic";
  return { sent, done: () => { delete e.RESEND_API_KEY; delete e.NTFY_TOPIC; spy.mockRestore(); } };
}

describe("ratings, tasks and builds", () => {
  it("rates and labels applications, sorts the best first and filters by label", async () => {
    const a = (await call("/api/apply", { body: tester({ nick: "Rated" }) })).json.id;
    await admin(`application/${a}/rate`, { rating: 5, tags: "Strong, active,strong" });
    const list = (await admin("applications?state=&sort=rating")).json.items as Json[];
    expect(list[0]).toMatchObject({ id: a, rating: 5, tags: "strong,active" });
    expect(((await admin("applications?state=&tag=active")).json.items as Json[]).map((x) => x.id)).toEqual([a]);
  });

  it("testers see open tasks and tick them off", async () => {
    const id = (await call("/api/apply", { body: tester() })).json.id;
    const code = (await admin(`application/${id}/decide`, { state: "accepted" })).json.code;
    const t = (await admin("task", { title: "Import a glossary", body: "From a big game" })).json.id;
    expect(((await call(`/api/tester/${code}`)).json.tasks as Json[]).find((x) => x.id === t)).toMatchObject({ title: "Import a glossary", state: null });
    expect((await call("/api/task", { body: { code, task: t, state: "problem", note: "It froze" } })).status).toBe(200);
    expect(((await call(`/api/tester/${code}`)).json.tasks as Json[]).find((x) => x.id === t)!.state).toBe("problem");
    expect(((await admin("tasks")).json.items as Json[]).find((x) => x.id === t)!.results[0]).toMatchObject({ state: "problem", note: "It froze" });
    await admin(`task/${t}`, { title: "Import a glossary", open: false });
    expect(((await call(`/api/tester/${code}`)).json.tasks as Json[]).some((x) => x.id === t)).toBe(false);
    expect((await call("/api/task", { body: { code, task: t, state: "done" } })).status).toBe(404);
  });

  it("a build uploaded in pieces downloads whole, only with a valid personal link, and is counted", async () => {
    const id = (await call("/api/apply", { body: tester() })).json.id;
    const code = (await admin(`application/${id}/decide`, { state: "accepted" })).json.code;
    const bytes = new Uint8Array(2500).map((_, i) => i % 251);
    const b64 = (u: Uint8Array) => btoa(String.fromCharCode(...u));
    const b = (await admin("build", { version: "2.0.0-beta.4", filename: "Aside setup.exe", size: bytes.length, audience: "test" })).json.id;
    await admin(`build/${b}/part`, { n: 0, data: b64(bytes.slice(0, 1200)) });
    expect((await admin(`build/${b}/finish`, {})).json.error).toBe("incomplete");
    await admin(`build/${b}/part`, { n: 1, data: b64(bytes.slice(1200)) });
    expect((await admin(`build/${b}/finish`, {})).json.ok).toBe(true);
    expect(((await call(`/api/tester/${code}`)).json.builds as Json[])[0]).toMatchObject({ id: b, version: "2.0.0-beta.4" });
    const dl = await call(`/api/build/${b}/${code}`);
    expect(dl.res.headers.get("content-disposition")).toContain("Aside setup.exe");
    expect(new Uint8Array(await dl.res.arrayBuffer())).toEqual(bytes);
    expect((await call(`/api/build/${b}/abcdefghjkmnpqrstuvwxyz2`)).status).toBe(404);
    expect(((await admin("builds")).json.items as Json[]).find((x) => x.id === b)!.downloads).toHaveLength(1);
    // a licence does not see a testers' build
    const lic = (await call("/api/apply", { body: licensee() })).json.id;
    const lcode = (await admin(`application/${lic}/decide`, { state: "accepted" })).json.code;
    expect((await call(`/api/build/${b}/${lcode}`)).status).toBe(404);
    // a link build redirects
    const l = (await admin("build", { version: "2.0.0-beta.5", url: "https://example.com/b5.exe", audience: "all" })).json.id;
    expect((await call(`/api/build/${l}/${lcode}`)).res.headers.get("location")).toBe("https://example.com/b5.exe");
    await admin(`build/${b}/delete`, {});
    expect((await call(`/api/build/${b}/${code}`)).status).toBe(404);
  });
});

describe("pages edited in the admin", () => {
  it("status, FAQ and blog posts", async () => {
    expect((await call("/api/status")).json.on).toBe(false);
    await admin("status", { on: true, build: "2.0.0-beta.4", next: "early November", note: "Testing the glossary." });
    expect((await call("/api/status")).json).toMatchObject({ on: true, build: "2.0.0-beta.4", next: "early November" });
    expect(((await call("/api/faq")).json.items as Json[]).length).toBe(6);
    await admin("faq", { q: "Is there a Mac version?", a: "Not yet.", sort: 70 });
    expect(((await call("/api/faq")).json.items as Json[]).at(-1)!.q).toBe("Is there a Mac version?");
    const post = (await admin("post", { title: "Glossary import, rebuilt", body: "First paragraph.\n\n### A heading\n\n- one\n- two", date: "2026-10-20" })).json.id;
    expect(post).toBe("glossary-import-rebuilt");
    expect((await call("/api/posts")).json.items).toHaveLength(0);
    await admin(`post/${post}`, { title: "Glossary import, rebuilt", body: "First paragraph.", date: "2026-10-20", published: true });
    expect(((await call("/api/posts")).json.items as Json[])[0].id).toBe(post);
  });
});

describe("the newsletter", () => {
  it("confirm, manage topics, unsubscribe; the newsletter goes only to confirmed people with an unsubscribe link", async () => {
    const out = fakeOutside();
    try {
      const email = `news${uid()}@example.com`;
      expect((await call("/api/subscribe", { body: { email, topics: ["releases"] } })).json).toMatchObject({ ok: true, confirm: true });
      const token = /#confirm=([a-z2-9]+)/.exec(out.sent.at(-1)!.body.text)![1]!;
      // asking again does not send another e-mail and does not tell whether the address is known
      const before = out.sent.length;
      expect((await call("/api/subscribe", { body: { email, topics: ["releases"] } })).json.ok).toBe(true);
      expect(out.sent.length).toBe(before);
      expect((await admin("subscribers")).json.counts.all).toBe(0);
      const manage = (await call("/api/subscribe/confirm", { body: { token } })).json.manage;
      expect((await admin("subscribers")).json.counts.releases).toBe(1);
      expect((await call(`/api/subscription/${manage}`)).json.topics).toEqual(["releases"]);
      await call(`/api/subscription/${manage}`, { body: { topics: ["releases", "diary"] } });
      expect((await admin("subscribers")).json.counts.diary).toBe(1);
      const r = await admin("newsletter/send", { topic: "diary", subject: "Hello", body: "News." });
      expect(r.json).toMatchObject({ sent: 1, total: 1 });
      const batch = out.sent.find((x) => x.url === "https://api.resend.com/emails/batch")!.body as Json[];
      expect(batch[0].headers["List-Unsubscribe"]).toContain(manage);
      expect(batch[0].text).toContain(`news.html#manage=${manage}`);
      await call(`/api/unsubscribe/${manage}`, { method: "POST" });
      expect((await admin("subscribers")).json.counts.all).toBe(0);
    } finally {
      out.done();
    }
  });
});

describe("health and phone notifications", () => {
  it("lists the checks, alerts once when e-mails keep failing, and says when they work again", async () => {
    const out = fakeOutside();
    try {
      const h = (await admin("health")).json;
      expect((h.checks as Json[]).map((c) => c.key)).toEqual(expect.arrayContaining(["database", "mail", "domain", "daily"]));
      expect((h.checks as Json[]).find((c) => c.key === "domain")!.ok).toBe(true);
      const now = Math.floor(Date.now() / 1000);
      for (let i = 0; i < 3; i++) await E.DB.prepare("INSERT INTO mails (created, to_email, kind, subject, body, sent) VALUES (?, 'x@example.com', 'test', 's', 'b', 0)").bind(now - 60).run();
      const { hourly } = await import("../src/cron.ts");
      const sentBefore = out.sent.filter((x) => x.url.startsWith("https://ntfy.sh/")).length;
      expect((await hourly(E, now)).alerts).toBe(1);
      expect((await hourly(E, now + 3600)).alerts).toBe(0); // not again within a day
      const pushes = out.sent.filter((x) => x.url.startsWith("https://ntfy.sh/"));
      expect(pushes.length).toBe(sentBefore + 1);
      expect(pushes.at(-1)!.headers.title).toContain("E-mail");
      await E.DB.prepare("DELETE FROM mails WHERE kind = 'test'").run();
      await hourly(E, now + 7200);
      expect(out.sent.filter((x) => x.url.startsWith("https://ntfy.sh/")).at(-1)!.headers.title).toContain("works again");
      expect((await admin("push/test", {})).json.sent).toBe(true);
    } finally {
      out.done();
    }
  });
});

describe("Cloudflare Access", () => {
  const b64 = (v: string | Uint8Array) => btoa(typeof v === "string" ? v : String.fromCharCode(...v)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  async function jwt(claims: Json, kid = "test-key"): Promise<string> {
    const key = await crypto.subtle.importKey("jwk", JSON.parse((env as unknown as { TEST_ACCESS_KEY: string }).TEST_ACCESS_KEY), { name: "RSASSA-PKCS1-v1_5", hash: "SHA-256" }, false, ["sign"]);
    const now = Math.floor(Date.now() / 1000);
    const head = b64(JSON.stringify({ alg: "RS256", kid, typ: "JWT" }));
    const body = b64(JSON.stringify({ aud: ["test-aud-0123"], iss: "https://aside-test.cloudflareaccess.com", email: "owner@example.com", iat: now, exp: now + 3600, ...claims }));
    const sig = new Uint8Array(await crypto.subtle.sign("RSASSA-PKCS1-v1_5", key, new TextEncoder().encode(head + "." + body)));
    return head + "." + body + "." + b64(sig);
  }
  const viaAccess = async (token: string) => call("/admin/api/overview", { headers: { "cf-access-jwt-assertion": token } });

  it("lets in a request signed by Access for this application", async () => {
    const r = await viaAccess(await jwt({}));
    expect(r.status).toBe(200);
    expect(r.json.via).toBe("access");
  });

  it("refuses another application, another team, an expired, a tampered or an unknown-key token", async () => {
    expect((await viaAccess(await jwt({ aud: ["other-app"] }))).status).toBe(401);
    expect((await viaAccess(await jwt({ iss: "https://evil.cloudflareaccess.com" }))).status).toBe(401);
    expect((await viaAccess(await jwt({ exp: Math.floor(Date.now() / 1000) - 10 }))).status).toBe(401);
    expect((await viaAccess(await jwt({}, "other-key"))).status).toBe(401);
    const good = await jwt({});
    const [h, , s] = good.split(".");
    const forged = h + "." + b64(JSON.stringify({ aud: ["test-aud-0123"], iss: "https://aside-test.cloudflareaccess.com", email: "x@y.z", exp: 9999999999 })) + "." + s;
    expect((await viaAccess(forged)).status).toBe(401);
  });

  it("without either, says which doors exist", async () => {
    const r = await call("/admin/api/overview");
    expect(r.status).toBe(401);
    expect(r.json).toMatchObject({ access: true, token: true });
  });
});

describe("mails", () => {
  it("escape what people typed", () => {
    const { html, text } = render({ subject: "Hi <b>", lines: ["<script>x</script>", "• one\n• two", "https://aside.test/report.html#abc"] });
    expect(html).not.toContain("<script>x");
    expect(html).toContain("&lt;script&gt;");
    expect(html).toContain('href="https://aside.test/report.html#abc"');
    expect(text).toContain("• two");
  });
});
