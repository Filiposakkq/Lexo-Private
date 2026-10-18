import type { Env } from "./env.ts";
import * as mail from "./mail.ts";
import { body, fail, human, ipHash, isEmail, isUrl, json, line, randomId, salt, sha256Hex, str, text, within } from "./util.ts";

const DAY = 86400;

// ---------------------------------------------------------------- settings and the round

export interface Program {
  round: string;
  open: boolean;
  closes: string;
  test: { places: number; taken: number };
  licence: { places: number; taken: number };
}

export async function settings(env: Env): Promise<Record<string, string>> {
  const rows = (await env.DB.prepare("SELECT key, value FROM settings").all<{ key: string; value: string }>()).results;
  return Object.fromEntries(rows.map((r) => [r.key, r.value]));
}

export async function program(env: Env): Promise<Program> {
  const s = await settings(env);
  const taken = (await env.DB.prepare("SELECT route, COUNT(*) AS n FROM applications WHERE state = 'accepted' GROUP BY route").all<{ route: string; n: number }>()).results;
  const n = (r: string) => taken.find((t) => t.route === r)?.n ?? 0;
  return {
    round: s.round ?? "Round 1",
    open: s.open !== "0",
    closes: s.closes ?? "",
    test: { places: Number(s.places_test ?? 4), taken: n("test") },
    licence: { places: Number(s.places_licence ?? 3), taken: n("licence") },
  };
}

export async function config(env: Env): Promise<Response> {
  return json({ ok: true, program: await program(env), turnstile: env.TURNSTILE_SITE_KEY ?? "" }, 200, { "cache-control": "public, max-age=60" });
}

// ---------------------------------------------------------------- beta applications

type Rule = { id: string; kind: "line" | "email" | "url" | "text" | "list"; min?: number; count?: number; when?: string; unless?: string };

/** What the server insists on (the page asks for the same, with friendlier words). */
export const RULES: Record<"test" | "licence", Rule[]> = {
  test: [
    { id: "nick", kind: "line" }, { id: "email", kind: "email" }, { id: "speak", kind: "list" }, { id: "win", kind: "line" }, { id: "ram", kind: "line" },
    { id: "exp", kind: "line" }, { id: "games", kind: "text", min: 15 }, { id: "bug", kind: "text", min: 80 }, { id: "hours", kind: "line" }, { id: "agree", kind: "list", count: 4 },
  ],
  licence: [
    { id: "nick", kind: "line" }, { id: "email", kind: "email" }, { id: "lang", kind: "line" }, { id: "game", kind: "line" },
    { id: "link", kind: "url", unless: "exception" }, { id: "why_ex", kind: "text", min: 60, when: "exception" },
    { id: "progress", kind: "line" }, { id: "size", kind: "line" }, { id: "team", kind: "line" }, { id: "tools", kind: "line" },
    { id: "why", kind: "text", min: 60 }, { id: "agree", kind: "list", count: 4 },
  ],
};

/** Returns the first rule the fields break, or null. */
export function check(route: "test" | "licence", f: Record<string, unknown>): string | null {
  for (const r of RULES[route]) {
    if (r.when && !f[r.when]) continue;
    if (r.unless && f[r.unless]) continue;
    const v = f[r.id];
    if (r.kind === "list") {
      if (!Array.isArray(v) || v.length < (r.count ?? 1)) return r.id;
      continue;
    }
    const s = str(v).trim();
    if (!s) return r.id;
    if (r.kind === "email" && !isEmail(s)) return r.id;
    if (r.kind === "url" && !isUrl(s)) return r.id;
    if (r.min && s.length < r.min) return r.id;
  }
  return null;
}

export async function apply(req: Request, env: Env, now: number): Promise<Response> {
  const b = await body(req, 60_000);
  if (!b) return fail("bad_request", 400);
  if (str(b.website)) return json({ ok: true, id: "x" }); // the hidden field only bots fill in
  const route = b.route === "test" || b.route === "licence" ? b.route : null;
  const f = (b.fields && typeof b.fields === "object" ? b.fields : {}) as Record<string, unknown>;
  if (!route) return fail("bad_request", 400);
  const bad = check(route, f);
  if (bad) return fail("missing", 422, { field: bad });
  const p = await program(env);
  if (!p.open) return fail("closed", 409);
  if (!(await human(env, b.turnstile, req))) return fail("not_human", 403);
  const ip = await ipHash(req, env);
  if (!(await within(env, ip, "apply", 5, DAY, now))) return fail("too_many", 429);
  // what the applicant saw, as label / value pairs, cleaned and limited
  const answers = (Array.isArray(b.answers) ? b.answers : [])
    .slice(0, 40)
    .map((a) => (Array.isArray(a) ? [line(a[0], 120), Array.isArray(a[1]) ? a[1].map((x) => line(x, 300)).slice(0, 12) : text(a[1], 1500)] : null))
    .filter(Boolean);
  const id = randomId(10);
  const nick = line(f.nick, 60);
  const email = line(f.email, 200);
  await env.DB.prepare("INSERT INTO applications (id, route, created, nick, email, lang, answers, ip_hash) VALUES (?,?,?,?,?,?,?,?)")
    .bind(id, route, now, nick, email, line(f.lang ?? (Array.isArray(f.speak) ? f.speak.join(", ") : ""), 120), JSON.stringify(answers), ip)
    .run();
  const left = route === "test" ? p.test.places - p.test.taken : p.licence.places - p.licence.taken;
  await mail.send(env, email, mail.received(route, nick, p.round, left <= 0));
  await mail.notifyOwner(env, `New application: ${route === "test" ? "testing" : "licence"} · ${nick}`, [
    `${nick} <${email}> applied for ${route === "test" ? "testing" : "a sponsored licence"} in ${p.round}.`,
    route === "licence" ? `Game: ${line(f.game, 120)} · ${line(f.lang, 60)}` + (f.exception ? " · asks for an exception" : "") : `Experience: ${line(f.exp, 80)}`,
  ]);
  return json({ ok: true, id, waitlist: left <= 0 });
}

// ---------------------------------------------------------------- roadmap and votes

const VOTER_RE = /^[0-9a-f]{32}$/;
const voterHash = async (env: Env, voter: string) => (await sha256Hex("voter:" + salt(env) + ":" + voter)).slice(0, 32);

export async function roadmapList(env: Env): Promise<{ id: string; col: string; title: string; text: string; tier: string; sort: number; votes: number }[]> {
  return (
    await env.DB.prepare(
      "SELECT r.id, r.col, r.title, r.text, r.tier, r.sort, r.extra + (SELECT COUNT(*) FROM votes v WHERE v.item_id = r.id) AS votes FROM roadmap r ORDER BY r.sort, r.created",
    ).all<{ id: string; col: string; title: string; text: string; tier: string; sort: number; votes: number }>()
  ).results;
}

export async function roadmap(req: Request, env: Env): Promise<Response> {
  const voter = new URL(req.url).searchParams.get("voter") ?? "";
  const items = await roadmapList(env);
  let mine: string[] = [];
  if (VOTER_RE.test(voter)) mine = (await env.DB.prepare("SELECT item_id FROM votes WHERE voter = ?").bind(await voterHash(env, voter)).all<{ item_id: string }>()).results.map((r) => r.item_id);
  return json({ ok: true, items: items.map((i) => ({ ...i, mine: mine.includes(i.id) })) });
}

export async function vote(req: Request, env: Env, now: number): Promise<Response> {
  const b = await body(req, 2000);
  const voter = str(b?.voter);
  const item = str(b?.item);
  if (!b || !VOTER_RE.test(voter) || !item) return fail("bad_request", 400);
  const row = await env.DB.prepare("SELECT col FROM roadmap WHERE id = ?").bind(item).first<{ col: string }>();
  if (!row || row.col === "now") return fail("not_found", 404);
  if (!(await within(env, await ipHash(req, env), "vote", 60, 3600, now))) return fail("too_many", 429);
  const h = await voterHash(env, voter);
  if (b.on === false) await env.DB.prepare("DELETE FROM votes WHERE item_id = ? AND voter = ?").bind(item, h).run();
  else await env.DB.prepare("INSERT OR IGNORE INTO votes (item_id, voter, at) VALUES (?, ?, ?)").bind(item, h, now).run();
  const c = await env.DB.prepare("SELECT r.extra + (SELECT COUNT(*) FROM votes v WHERE v.item_id = r.id) AS votes FROM roadmap r WHERE r.id = ?").bind(item).first<{ votes: number }>();
  return json({ ok: true, votes: c?.votes ?? 0, mine: b.on !== false });
}

// ---------------------------------------------------------------- the personal pages: feedback (licences) and bug reports (testers)

interface AppRow {
  id: string; route: string; nick: string; email: string; state: string; code: string; answers: string; trial_start: number | null;
  fb2_sent: number | null; fb2_done: number | null; fb4_sent: number | null; fb4_done: number | null;
}
const CODE_RE = /^[a-z2-9]{16,40}$/;
const byCode = (env: Env, code: string) =>
  CODE_RE.test(code) ? env.DB.prepare("SELECT * FROM applications WHERE code = ? AND state = 'accepted'").bind(code).first<AppRow>() : Promise.resolve(null);

const gameOf = (a: AppRow) => {
  try {
    const found = (JSON.parse(a.answers) as [string, unknown][]).find((x) => /^game/i.test(x[0]));
    return found ? String(found[1]) : "";
  } catch {
    return "";
  }
};

export async function feedbackInfo(env: Env, code: string, now: number): Promise<Response> {
  const a = await byCode(env, code);
  if (!a || a.route !== "licence") return fail("not_found", 404);
  const next = !a.fb2_done ? "2w" : !a.fb4_done ? "4w" : null;
  const day = a.trial_start ? Math.floor((now - a.trial_start) / DAY) : null;
  return json({ ok: true, nick: a.nick, game: gameOf(a), next, done: { "2w": !!a.fb2_done, "4w": !!a.fb4_done }, day });
}

export async function feedbackSend(req: Request, env: Env, code: string, now: number): Promise<Response> {
  const a = await byCode(env, code);
  if (!a || a.route !== "licence") return fail("not_found", 404);
  const b = await body(req, 30_000);
  const which = b?.which === "2w" || b?.which === "4w" ? b.which : null;
  if (!b || !which) return fail("bad_request", 400);
  if ((which === "2w" && a.fb2_done) || (which === "4w" && a.fb4_done)) return fail("already_sent", 409);
  if (which === "4w" && !a.fb2_done) return fail("bad_request", 400);
  const answers = (Array.isArray(b.answers) ? b.answers : []).slice(0, 20).map((x) => (Array.isArray(x) ? [line(x[0], 120), text(x[1], 2000)] : null)).filter(Boolean);
  if (answers.length < 3) return fail("missing", 422);
  if (!(await within(env, await ipHash(req, env), "feedback", 10, DAY, now))) return fail("too_many", 429);
  await env.DB.prepare("INSERT INTO feedback (application_id, which, created, answers) VALUES (?,?,?,?)").bind(a.id, which, now, JSON.stringify(answers)).run();
  await env.DB.prepare(`UPDATE applications SET ${which === "2w" ? "fb2_done" : "fb4_done"} = ? WHERE id = ?`).bind(now, a.id).run();
  await mail.notifyOwner(env, `Feedback (${which === "2w" ? "2 weeks" : "4 weeks"}) · ${a.nick}`, [
    `${a.nick} sent the ${which === "2w" ? "2-week" : "4-week"} feedback.` + (which === "4w" ? " Both are in: the full licence can be issued." : ""),
  ]);
  return json({ ok: true });
}

export async function testerInfo(env: Env, code: string): Promise<Response> {
  const a = await byCode(env, code);
  if (!a) return fail("not_found", 404);
  const bugs = (await env.DB.prepare("SELECT id, created, title, severity, state FROM bugs WHERE application_id = ? ORDER BY id DESC LIMIT 50").bind(a.id).all()).results;
  return json({ ok: true, nick: a.nick, bugs });
}

const IMAGE_RE = /^data:(image\/(?:png|jpeg|webp));base64,([A-Za-z0-9+/=]+)$/;

export async function bugSend(req: Request, env: Env, now: number): Promise<Response> {
  const b = await body(req, 2_000_000);
  const a = await byCode(env, str(b?.code));
  if (!b || !a) return fail("not_found", 404);
  const title = line(b.title, 140);
  const steps = text(b.steps, 4000);
  if (title.length < 5 || steps.length < 20) return fail("missing", 422, { field: title.length < 5 ? "title" : "steps" });
  if (!(await within(env, "code:" + a.id, "bug", 30, DAY, now))) return fail("too_many", 429);
  let image: Uint8Array | null = null;
  let type = "";
  const m = IMAGE_RE.exec(str(b.image));
  if (m) {
    const bin = atob(m[2]!);
    if (bin.length > 1_400_000) return fail("image_too_big", 413);
    image = Uint8Array.from(bin, (c) => c.charCodeAt(0));
    type = m[1]!;
  }
  const severity = ["crash", "broken", "normal", "cosmetic"].includes(str(b.severity)) ? str(b.severity) : "normal";
  const r = await env.DB.prepare("INSERT INTO bugs (application_id, created, title, steps, expected, actual, version, severity, image, image_type) VALUES (?,?,?,?,?,?,?,?,?,?) RETURNING id")
    .bind(a.id, now, title, steps, text(b.expected, 2000), text(b.actual, 2000), line(b.version, 40), severity, image, type)
    .first<{ id: number }>();
  await mail.notifyOwner(env, `Bug #${r?.id} (${severity}) · ${title}`, [`${a.nick} reported: ${title}`, steps.slice(0, 600)]);
  return json({ ok: true, id: r?.id });
}

// ---------------------------------------------------------------- showcase, contact, releases

export async function showcaseList(env: Env): Promise<Response> {
  const items = (await env.DB.prepare("SELECT id, game, lang, link, translator, blurb, lines FROM showcase WHERE state = 'approved' ORDER BY sort, created").all()).results;
  return json({ ok: true, items }, 200, { "cache-control": "public, max-age=120" });
}

export async function showcaseSend(req: Request, env: Env, now: number): Promise<Response> {
  const b = await body(req, 20_000);
  if (!b) return fail("bad_request", 400);
  if (str(b.website)) return json({ ok: true });
  const game = line(b.game, 100), lang = line(b.lang, 60), link = line(b.link, 300), translator = line(b.translator, 80), email = line(b.email, 200);
  if (!game || !lang || !translator) return fail("missing", 422, { field: !game ? "game" : !lang ? "lang" : "translator" });
  if (!isUrl(link)) return fail("missing", 422, { field: "link" });
  if (!isEmail(email)) return fail("missing", 422, { field: "email" });
  if (!(await human(env, b.turnstile, req))) return fail("not_human", 403);
  const ip = await ipHash(req, env);
  if (!(await within(env, ip, "showcase", 5, DAY, now))) return fail("too_many", 429);
  const id = randomId(10);
  await env.DB.prepare("INSERT INTO showcase (id, created, game, lang, link, translator, email, blurb, lines, ip_hash, sort) VALUES (?,?,?,?,?,?,?,?,?,?,100)")
    .bind(id, now, game, lang, link, translator, email, text(b.blurb, 400), line(b.lines, 20), ip)
    .run();
  await mail.notifyOwner(env, `Showcase entry · ${game} (${lang})`, [`${translator} <${email}> sent ${game} in ${lang}: ${link}`]);
  return json({ ok: true });
}

export async function contact(req: Request, env: Env, now: number): Promise<Response> {
  const b = await body(req, 20_000);
  if (!b) return fail("bad_request", 400);
  if (str(b.website)) return json({ ok: true });
  const name = line(b.name, 80), email = line(b.email, 200), topic = line(b.topic, 60), msg = text(b.message, 5000);
  if (!name) return fail("missing", 422, { field: "name" });
  if (!isEmail(email)) return fail("missing", 422, { field: "email" });
  if (msg.length < 10) return fail("missing", 422, { field: "message" });
  if (!(await human(env, b.turnstile, req))) return fail("not_human", 403);
  const ip = await ipHash(req, env);
  if (!(await within(env, ip, "contact", 5, 3600, now))) return fail("too_many", 429);
  const r = await env.DB.prepare("INSERT INTO messages (created, name, email, topic, body, ip_hash) VALUES (?,?,?,?,?,?) RETURNING id").bind(now, name, email, topic, msg, ip).first<{ id: number }>();
  await mail.notifyOwner(env, `Message${topic ? " · " + topic : ""} · ${name}`, [`${name} <${email}> wrote:`, msg.slice(0, 2000)]);
  return json({ ok: true, id: r?.id });
}

export async function releases(env: Env): Promise<Response> {
  const items = (await env.DB.prepare("SELECT id, version, date, channel, title, notes, sha256, size, url != '' AS downloadable FROM releases WHERE published = 1 ORDER BY date DESC, created DESC").all()).results;
  return json({ ok: true, items }, 200, { "cache-control": "public, max-age=120" });
}

export async function download(env: Env, id: string): Promise<Response> {
  const row =
    id === "latest"
      ? await env.DB.prepare("SELECT id, url FROM releases WHERE published = 1 AND channel = 'stable' AND url != '' ORDER BY date DESC, created DESC LIMIT 1").first<{ id: string; url: string }>()
      : await env.DB.prepare("SELECT id, url FROM releases WHERE id = ? AND published = 1 AND url != ''").bind(id).first<{ id: string; url: string }>();
  if (!row || !/^https:\/\//.test(row.url)) return fail("not_found", 404);
  await env.DB.prepare("UPDATE releases SET downloads = downloads + 1 WHERE id = ?").bind(row.id).run();
  return Response.redirect(row.url, 302);
}
