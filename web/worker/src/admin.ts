import type { Env } from "./env.ts";
import * as mail from "./mail.ts";
import { program, roadmapList, settings } from "./public.ts";
import { adminConfigured, admitted } from "./access.ts";
import { body, fail, isEmail, isUrl, json, line, randomId, site, str, text } from "./util.ts";
import { health } from "./cron.ts";
import { EVENTS, push, pushConfigured } from "./notify.ts";
import { TOPICS } from "./program.ts";

const DAY = 86400;
const all = async <T = Record<string, unknown>>(env: Env, sql: string, ...args: unknown[]) => (await env.DB.prepare(sql).bind(...args).all<T>()).results;
const one = <T = Record<string, unknown>>(env: Env, sql: string, ...args: unknown[]) => env.DB.prepare(sql).bind(...args).first<T>();
const run = (env: Env, sql: string, ...args: unknown[]) => env.DB.prepare(sql).bind(...args).run();
const pick = (v: unknown, allowed: string[], fallback: string) => (allowed.includes(str(v)) ? str(v) : fallback);
const count = async (env: Env, sql: string, ...args: unknown[]) => (await one<{ n: number }>(env, sql, ...args))?.n ?? 0;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

interface App { id: string; route: "test" | "licence"; nick: string; email: string; state: string; code: string | null; trial_start: number | null; ip_hash: string }

const BUG_STATES = ["new", "confirmed", "fixed", "cant-reproduce", "duplicate"];
const BUG_LABEL: Record<string, string> = { new: "new", confirmed: "confirmed", fixed: "fixed", "cant-reproduce": "can't reproduce", duplicate: "duplicate" };

/** What can be deleted in bulk: the table and what goes with a row. */
const KINDS: Record<string, { table: string; key: "id" | "num" }> = {
  applications: { table: "applications", key: "id" },
  bugs: { table: "bugs", key: "num" },
  messages: { table: "messages", key: "num" },
  showcase: { table: "showcase", key: "id" },
  mails: { table: "mails", key: "num" },
  feedback: { table: "feedback", key: "num" },
  releases: { table: "releases", key: "id" },
  log: { table: "log", key: "num" },
  subscribers: { table: "subscribers", key: "num" },
  tasks: { table: "tasks", key: "id" },
  builds: { table: "builds", key: "id" },
  posts: { table: "posts", key: "id" },
  faq: { table: "faq", key: "id" },
};

/** An application with everything that belongs to it. */
async function deleteApplication(env: Env, id: string): Promise<void> {
  await run(env, "DELETE FROM feedback WHERE application_id = ?", id);
  await run(env, "DELETE FROM bugs WHERE application_id = ?", id);
  await run(env, "DELETE FROM mails WHERE application_id = ?", id);
  await run(env, "DELETE FROM task_results WHERE application_id = ?", id);
  await run(env, "DELETE FROM build_downloads WHERE application_id = ?", id);
  await run(env, "DELETE FROM applications WHERE id = ?", id);
}

async function deleteBuild(env: Env, id: string): Promise<void> {
  await run(env, "DELETE FROM build_parts WHERE build_id = ?", id);
  await run(env, "DELETE FROM build_downloads WHERE build_id = ?", id);
  await run(env, "DELETE FROM builds WHERE id = ?", id);
}

/** One e-mail to each person of a group, with {nick} and {link} filled in. */
async function sendToGroup(env: Env, to: string, subject: string, bodyText: string, kind: string): Promise<{ sent: number; total: number }> {
  const people = await recipients(env, to);
  let sent = 0;
  for (let i = 0; i < people.length; i++) {
    const x = people[i]!;
    if (i) await sleep(600); // Resend lets a free account send about two e-mails a second
    if (await mail.send(env, x.email, mail.custom(subject, bodyText, x.nick, mail.pageLink(env, x.route, x.code)), { kind, app: x.id })) sent++;
  }
  return { sent, total: people.length };
}

const setSetting = (env: Env, k: string, v: string) => run(env, "INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value", k, v);
const slug = (t: string) => t.toLowerCase().normalize("NFD").replace(/[\u0300-\u036f]/g, "").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 50);

async function deleteFeedback(env: Env, id: number): Promise<void> {
  const f = await one<{ application_id: string; which: string }>(env, "SELECT application_id, which FROM feedback WHERE id = ?", id);
  if (!f) return;
  await run(env, "DELETE FROM feedback WHERE id = ?", id);
  await run(env, `UPDATE applications SET ${f.which === "2w" ? "fb2_done" : "fb4_done"} = NULL WHERE id = ?`, f.application_id);
}

/** The people a group mail goes to. */
async function recipients(env: Env, to: string): Promise<App[]> {
  const cols = "id, route, nick, email, state, code, trial_start, ip_hash";
  if (to.startsWith("app:")) return all<App>(env, `SELECT ${cols} FROM applications WHERE id = ?`, to.slice(4));
  if (to === "test" || to === "licence") return all<App>(env, `SELECT ${cols} FROM applications WHERE state = 'accepted' AND route = ?`, to);
  if (to === "accepted") return all<App>(env, `SELECT ${cols} FROM applications WHERE state = 'accepted'`);
  if (to === "waitlist") return all<App>(env, `SELECT ${cols} FROM applications WHERE state = 'waitlist' AND verified IS NOT NULL`);
  return [];
}

/** CSV with a header row; every value quoted. */
function csv(rows: Record<string, unknown>[]): string {
  if (!rows.length) return "";
  const cols = Object.keys(rows[0]!);
  const q = (v: unknown) => '"' + String(v ?? "").replace(/"/g, '""') + '"';
  return [cols.map(q).join(","), ...rows.map((r) => cols.map((c) => q(r[c])).join(","))].join("\r\n") + "\r\n";
}

const iso = (s: unknown) => (typeof s === "number" && s > 0 ? new Date(s * 1000).toISOString().slice(0, 16).replace("T", " ") : "");

/** `/admin/api/...`: every call needs a valid Cloudflare Access token, or `Authorization: Bearer <ADMIN_TOKEN>` (src/access.ts). */
export async function adminApi(req: Request, env: Env, now: number, path: string): Promise<Response> {
  if (!adminConfigured(env)) return fail("admin_not_configured", 503);
  const who = await admitted(req, env, now);
  if (!who) return fail("unauthorised", 401, { access: !!(env.ACCESS_TEAM_DOMAIN && env.ACCESS_AUD), token: !!env.ADMIN_TOKEN });
  const via = who === "token" ? "token" : "access";
  const url = new URL(req.url);
  const [a, b, c] = path.split("/").filter(Boolean);
  const q = (k: string) => url.searchParams.get(k) ?? "";
  const log = (action: string, target = "", detail = "") => run(env, "INSERT INTO log (at, who, action, target, detail) VALUES (?,?,?,?,?)", now, who, action, target, detail.slice(0, 500));

  if (req.method === "GET") {
    switch (a) {
      case "overview": {
        const n = (sql: string) => count(env, sql);
        const size = (await env.DB.prepare("SELECT 1").run()).meta.size_after ?? 0;
        return json({
          ok: true,
          via,
          who,
          program: await program(env),
          mail: !!env.RESEND_API_KEY,
          turnstile: !!env.TURNSTILE_SECRET,
          pages_open: (await settings(env)).pages_open !== "0",
          size,
          counts: {
            applications: await n("SELECT COUNT(*) AS n FROM applications WHERE state = 'new' AND verified IS NOT NULL"),
            unconfirmed: await n("SELECT COUNT(*) AS n FROM applications WHERE verified IS NULL"),
            people: await n("SELECT COUNT(*) AS n FROM applications WHERE state = 'accepted'"),
            feedback_due: await n("SELECT COUNT(*) AS n FROM applications WHERE route = 'licence' AND state = 'accepted' AND ((fb2_sent IS NOT NULL AND fb2_done IS NULL) OR (fb4_sent IS NOT NULL AND fb4_done IS NULL))"),
            ready: await n("SELECT COUNT(*) AS n FROM applications WHERE route = 'licence' AND state = 'accepted' AND fb4_done IS NOT NULL"),
            bugs: await n("SELECT COUNT(*) AS n FROM bugs WHERE state = 'new'"),
            messages: await n("SELECT COUNT(*) AS n FROM messages WHERE state = 'new'"),
            showcase: await n("SELECT COUNT(*) AS n FROM showcase WHERE state = 'pending'"),
            failed_mails: await n(`SELECT COUNT(*) AS n FROM mails WHERE sent = 0 AND created > ${now - 7 * DAY}`),
            votes: await n("SELECT COUNT(*) AS n FROM votes"),
            subscribers: await n("SELECT COUNT(*) AS n FROM subscribers WHERE confirmed IS NOT NULL"),
            task_problems: await n(`SELECT COUNT(*) AS n FROM task_results r JOIN tasks t ON t.id = r.task_id WHERE t.open = 1 AND r.state = 'problem'`),
            downloads: await n("SELECT COALESCE(SUM(downloads), 0) AS n FROM releases"),
          },
          recent: await all(env, `SELECT * FROM (
              SELECT 'application' AS kind, id, created, nick AS who, route AS detail FROM applications WHERE verified IS NOT NULL
              UNION ALL SELECT 'bug', CAST(id AS TEXT), created, title, severity FROM bugs
              UNION ALL SELECT 'message', CAST(id AS TEXT), created, name, topic FROM messages
              UNION ALL SELECT 'showcase', id, created, translator, game FROM showcase WHERE created > 0
              UNION ALL SELECT 'feedback', f.application_id, f.created, a.nick, f.which FROM feedback f JOIN applications a ON a.id = f.application_id
            ) ORDER BY created DESC LIMIT 15`),
        });
      }
      case "applications": {
        const state = q("state");
        const where = state === "unconfirmed" ? "verified IS NULL" : "verified IS NOT NULL AND (? = '' OR state = ?)";
        const args = state === "unconfirmed" ? [] : [state, state];
        return json({
          ok: true,
          items: await all(env, `SELECT id, route, created, nick, email, lang, state, verified, trial_start, fb2_sent, fb2_done, fb4_sent, fb4_done, ip_hash, rating, tags,
              (SELECT COUNT(*) FROM applications x WHERE x.ip_hash = applications.ip_hash AND x.ip_hash != '' AND x.id != applications.id) AS same_ip
            FROM applications WHERE (? = '' OR route = ?) AND ${where} AND (? = '' OR (',' || tags || ',') LIKE '%,' || ? || ',%')
            ORDER BY ${q("sort") === "rating" ? "rating DESC," : ""} CASE state WHEN 'new' THEN 0 WHEN 'waitlist' THEN 1 WHEN 'accepted' THEN 2 ELSE 3 END, created DESC LIMIT 300`, q("route"), q("route"), ...args, q("tag"), q("tag")),
        });
      }
      case "people":
        return json({
          ok: true,
          items: await all(env, `SELECT a.id, a.route, a.nick, a.email, a.lang, a.state, a.decided, a.last_seen, a.trial_start, a.fb2_sent, a.fb2_done, a.fb4_sent, a.fb4_done, a.removed_reason, a.code IS NOT NULL AS has_link,
              (SELECT COUNT(*) FROM bugs b WHERE b.application_id = a.id) AS bugs,
              (SELECT COUNT(*) FROM bugs b WHERE b.application_id = a.id AND b.state IN ('new', 'confirmed')) AS open_bugs,
              (SELECT MAX(created) FROM mails m WHERE m.application_id = a.id) AS last_mail
            FROM applications a WHERE a.state IN ('accepted', 'removed') AND (? = '' OR a.route = ?)
            ORDER BY a.state = 'removed', a.route, a.decided`, q("route"), q("route")),
        });
      case "application": {
        const app = await one<Record<string, unknown>>(env, "SELECT * FROM applications WHERE id = ?", b);
        if (!app) return fail("not_found", 404);
        const ip = str(app.ip_hash);
        delete app.ip_hash;
        delete app.verify_token;
        return json({
          ok: true,
          item: app,
          link: mail.pageLink(env, str(app.route), (app.code as string | null) ?? null),
          same_ip: ip ? await count(env, "SELECT COUNT(*) AS n FROM applications WHERE ip_hash = ? AND id != ?", ip, b) : 0,
          feedback: await all(env, "SELECT * FROM feedback WHERE application_id = ? ORDER BY created", b),
          bugs: await all(env, "SELECT id, created, title, state, severity FROM bugs WHERE application_id = ? ORDER BY id DESC", b),
          mails: await all(env, "SELECT id, created, kind, subject, body, sent FROM mails WHERE application_id = ? ORDER BY created DESC LIMIT 100", b),
          log: await all(env, "SELECT at, who, action, detail FROM log WHERE target = ? ORDER BY at DESC LIMIT 100", b),
        });
      }
      case "settings":
        return json({ ok: true, settings: await settings(env), program: await program(env) });
      case "roadmap":
        return json({ ok: true, items: await all(env, "SELECT r.*, (SELECT COUNT(*) FROM votes v WHERE v.item_id = r.id) AS live FROM roadmap r ORDER BY r.sort, r.created") });
      case "bugs":
        return json({
          ok: true,
          items: await all(env, `SELECT b.id, b.created, b.title, b.steps, b.expected, b.actual, b.version, b.severity, b.state, b.note, b.reply, b.replied, b.closed, b.image_type,
              COALESCE(length(b.image), 0) AS image_size, b.application_id, a.nick, a.email
            FROM bugs b JOIN applications a ON a.id = b.application_id
            WHERE (? = '' OR b.state = ? OR (? = 'open' AND b.state IN ('new', 'confirmed'))) AND (? = '' OR b.severity = ?) AND (? = '' OR b.application_id = ?)
            ORDER BY b.id DESC LIMIT 300`, q("state"), q("state"), q("state"), q("severity"), q("severity"), q("app"), q("app")),
        });
      case "bug": {
        if (c !== "image") break;
        const r = await one<{ image: ArrayBuffer | number[] | null; image_type: string }>(env, "SELECT image, image_type FROM bugs WHERE id = ?", Number(b));
        if (!r?.image) return fail("not_found", 404);
        // D1 hands a BLOB over as an array of numbers
        return new Response(r.image instanceof ArrayBuffer ? r.image : new Uint8Array(r.image), { headers: { "content-type": r.image_type, "cache-control": "private, max-age=3600", "content-security-policy": "default-src 'none'" } });
      }
      case "showcase":
        return json({ ok: true, items: await all(env, "SELECT id, created, state, game, lang, link, translator, email, blurb, lines, sort FROM showcase ORDER BY CASE state WHEN 'pending' THEN 0 WHEN 'approved' THEN 1 ELSE 2 END, sort, created DESC") });
      case "messages":
        return json({ ok: true, items: await all(env, "SELECT id, created, name, email, topic, body, state FROM messages WHERE (? = '' OR state = ?) ORDER BY id DESC LIMIT 300", q("state"), q("state")) });
      case "releases":
        return json({ ok: true, items: await all(env, "SELECT * FROM releases ORDER BY date DESC, created DESC") });
      case "feedback":
        return json({ ok: true, items: await all(env, "SELECT f.*, a.nick, a.email FROM feedback f JOIN applications a ON a.id = f.application_id ORDER BY f.created DESC LIMIT 200") });
      case "mails":
        return json({
          ok: true,
          items: await all(env, `SELECT m.id, m.created, m.application_id, m.to_email, m.kind, m.subject, m.body, m.sent, a.nick FROM mails m LEFT JOIN applications a ON a.id = m.application_id
            WHERE (? = '' OR m.kind = ?) AND (? = '' OR (? = 'failed' AND m.sent = 0)) ORDER BY m.id DESC LIMIT 300`, q("kind"), q("kind"), q("only"), q("only")),
          templates: await all(env, "SELECT * FROM templates ORDER BY created, name"),
        });
      case "templates":
        return json({ ok: true, items: await all(env, "SELECT * FROM templates ORDER BY created, name") });
      case "log":
        return json({ ok: true, items: await all(env, "SELECT * FROM log ORDER BY id DESC LIMIT 300") });
      case "storage": {
        const tables = ["applications", "feedback", "bugs", "mails", "messages", "showcase", "roadmap", "votes", "releases", "log", "hits", "subscribers", "tasks", "builds", "posts"];
        const rows: Record<string, number> = {};
        for (const t of tables) rows[t] = await count(env, `SELECT COUNT(*) AS n FROM ${t}`);
        const images = await one<{ n: number; bytes: number }>(env, "SELECT COUNT(*) AS n, COALESCE(SUM(length(image)), 0) AS bytes FROM bugs WHERE image IS NOT NULL");
        const closedImages = await one<{ n: number; bytes: number }>(env, `SELECT COUNT(*) AS n, COALESCE(SUM(length(image)), 0) AS bytes FROM bugs WHERE image IS NOT NULL AND state IN ('fixed', 'cant-reproduce', 'duplicate')`);
        const size = (await env.DB.prepare("SELECT 1").run()).meta.size_after ?? 0;
        return json({
          ok: true,
          size,
          limit: 500 * 1024 * 1024,
          rows,
          images,
          closed_images: closedImages,
          declined: await count(env, "SELECT COUNT(*) AS n FROM applications WHERE state = 'declined'"),
          unconfirmed: await count(env, "SELECT COUNT(*) AS n FROM applications WHERE verified IS NULL"),
          archived: await count(env, "SELECT COUNT(*) AS n FROM messages WHERE state = 'archived'"),
          builds_bytes: await count(env, "SELECT COALESCE(SUM(length(data)), 0) AS n FROM build_parts"),
        });
      }
      case "search": {
        const term = q("q").trim().slice(0, 80);
        if (term.length < 2) return json({ ok: true, items: [] });
        const like = "%" + term.replace(/[%_\\]/g, (m) => "\\" + m) + "%";
        const items = await all(env, `SELECT * FROM (
            SELECT 'application' AS kind, id, nick AS title, email || ' · ' || route || ' · ' || state AS detail, created FROM applications
              WHERE nick LIKE ?1 ESCAPE '\\' OR email LIKE ?1 ESCAPE '\\' OR answers LIKE ?1 ESCAPE '\\' OR note LIKE ?1 ESCAPE '\\'
            UNION ALL SELECT 'bug', CAST(id AS TEXT), '#' || id || ' ' || title, state, created FROM bugs WHERE title LIKE ?1 ESCAPE '\\' OR steps LIKE ?1 ESCAPE '\\' OR note LIKE ?1 ESCAPE '\\'
            UNION ALL SELECT 'message', CAST(id AS TEXT), name, email || ' · ' || substr(body, 1, 80), created FROM messages WHERE name LIKE ?1 ESCAPE '\\' OR email LIKE ?1 ESCAPE '\\' OR body LIKE ?1 ESCAPE '\\'
            UNION ALL SELECT 'showcase', id, game || ' · ' || lang, translator, created FROM showcase WHERE game LIKE ?1 ESCAPE '\\' OR translator LIKE ?1 ESCAPE '\\' OR email LIKE ?1 ESCAPE '\\'
            UNION ALL SELECT 'mail', CAST(id AS TEXT), subject, to_email, created FROM mails WHERE to_email LIKE ?1 ESCAPE '\\' OR subject LIKE ?1 ESCAPE '\\'
          ) ORDER BY created DESC LIMIT 50`, like);
        return json({ ok: true, items });
      }
      case "health":
        return json({
          ok: true,
          checks: await health(env, now),
          push: pushConfigured(env),
          events: Object.entries(EVENTS).map(([key, label]) => ({ key, label })),
          on: ((await settings(env)).push_events ?? "").split(",").filter(Boolean),
        });
      case "tasks": {
        const tasks = await all<Record<string, unknown>>(env, "SELECT * FROM tasks ORDER BY open DESC, sort, created DESC");
        const results = await all<{ task_id: string }>(env, "SELECT r.task_id, r.state, r.note, r.at, r.application_id, a.nick FROM task_results r JOIN applications a ON a.id = r.application_id ORDER BY r.at DESC");
        const testers = await count(env, "SELECT COUNT(*) AS n FROM applications WHERE state = 'accepted' AND route = 'test'");
        return json({ ok: true, testers, items: tasks.map((t) => ({ ...t, results: results.filter((r) => r.task_id === t.id) })) });
      }
      case "builds": {
        const builds = await all<Record<string, unknown>>(env, "SELECT id, created, version, notes, audience, url, filename, size, sha256, parts, ready, visible FROM builds ORDER BY created DESC");
        const dl = await all<{ build_id: string }>(env, "SELECT d.build_id, d.at, a.nick, a.id AS app FROM build_downloads d JOIN applications a ON a.id = d.application_id ORDER BY d.at DESC");
        return json({ ok: true, items: builds.map((x) => ({ ...x, downloads: dl.filter((d) => d.build_id === x.id) })) });
      }
      case "subscribers": {
        const items = await all(env, "SELECT id, created, email, topics, confirmed FROM subscribers WHERE (? = '' OR (',' || topics || ',') LIKE '%,' || ? || ',%') ORDER BY confirmed IS NULL, created DESC LIMIT 1000", q("topic"), q("topic"));
        const counts: Record<string, number> = {};
        for (const t of TOPICS) counts[t] = await count(env, "SELECT COUNT(*) AS n FROM subscribers WHERE confirmed IS NOT NULL AND (',' || topics || ',') LIKE ?", `%,${t},%`);
        counts.all = await count(env, "SELECT COUNT(*) AS n FROM subscribers WHERE confirmed IS NOT NULL");
        return json({ ok: true, items, counts, sent: await all(env, "SELECT * FROM newsletters ORDER BY id DESC LIMIT 50") });
      }
      case "faq":
        return json({ ok: true, items: await all(env, "SELECT * FROM faq ORDER BY sort, id") });
      case "posts":
        return json({ ok: true, items: await all(env, "SELECT * FROM posts ORDER BY date DESC, created DESC") });
      case "export": {
        const what = b ?? "";
        const day = new Date(now * 1000).toISOString().slice(0, 10);
        if (what === "backup") {
          // everything except screenshots and the flood counters, as one JSON file
          const out: Record<string, unknown> = { exported: day };
          for (const t of ["settings", "applications", "feedback", "roadmap", "showcase", "messages", "releases", "mails", "templates", "log", "tasks", "task_results", "subscribers", "newsletters", "faq", "posts"]) out[t] = await all(env, `SELECT * FROM ${t}`);
          out.bugs = await all(env, "SELECT id, application_id, created, title, steps, expected, actual, version, severity, state, note, reply, replied, closed, image_type FROM bugs");
          await log("export", "backup");
          return new Response(JSON.stringify(out, null, 1), { headers: { "content-type": "application/json", "content-disposition": `attachment; filename="aside-web-${day}.json"`, "cache-control": "no-store" } });
        }
        const sets: Record<string, string> = {
          applications: "SELECT id, route, state, nick, email, lang, created, verified, decided, note FROM applications ORDER BY created",
          people: "SELECT id, route, state, nick, email, lang, decided, last_seen, trial_start, fb2_done, fb4_done, removed_reason FROM applications WHERE state IN ('accepted', 'removed') ORDER BY route, decided",
          bugs: "SELECT b.id, a.nick, b.created, b.severity, b.state, b.version, b.title, b.steps, b.expected, b.actual, b.reply, b.note FROM bugs b JOIN applications a ON a.id = b.application_id ORDER BY b.id",
          messages: "SELECT id, created, name, email, topic, state, body FROM messages ORDER BY id",
          mails: "SELECT id, created, to_email, kind, sent, subject FROM mails ORDER BY id",
          subscribers: "SELECT id, created, email, topics, confirmed FROM subscribers ORDER BY id",
        };
        if (!sets[what]) break;
        const rows = (await all<Record<string, unknown>>(env, sets[what]!)).map((r) => {
          for (const k of ["created", "verified", "decided", "last_seen", "trial_start", "fb2_done", "fb4_done", "confirmed"]) if (k in r) r[k] = iso(r[k]);
          return r;
        });
        return new Response("﻿" + csv(rows), { headers: { "content-type": "text/csv; charset=utf-8", "content-disposition": `attachment; filename="aside-${what}-${day}.csv"`, "cache-control": "no-store" } });
      }
    }
    return fail("not_found", 404);
  }

  if (req.method !== "POST") return fail("not_found", 404);
  const p = (await body(req, a === "build" && c === "part" ? 1_600_000 : 200_000)) ?? {};

  switch (a) {
    case "application": {
      const app = await one<App>(env, "SELECT id, route, nick, email, state, code, trial_start, ip_hash FROM applications WHERE id = ?", b);
      if (!app) return fail("not_found", 404);
      if (c === "decide") {
        // accepted and removed people are handled by remove / readmit, so a click cannot drop someone by mistake
        const state = pick(p.state, ["accepted", "waitlist", "declined", "new"], "");
        if (!state) return fail("bad_request", 400);
        if ((app.state === "accepted" || app.state === "removed") && state !== app.state) return fail("use_remove", 409);
        const code = state === "accepted" ? app.code ?? randomId(24) : app.code;
        const trial = state === "accepted" && app.route === "licence" && p.start_trial !== false ? app.trial_start ?? now : app.trial_start;
        await run(env, "UPDATE applications SET state = ?, decided = ?, code = ?, trial_start = ?, verified = COALESCE(verified, ?), verify_token = NULL WHERE id = ?", state, now, code, trial, now, app.id);
        let mailed = false;
        if (p.send === true && state !== "new")
          mailed = await mail.send(env, app.email, mail.decision(app.route, state as "accepted", app.nick, code ?? "", env, text(p.message, 2000)), { kind: "decision", app: app.id });
        await log("decide", app.id, `${app.nick}: ${state}${p.send === true ? (mailed ? ", e-mail sent" : ", e-mail failed") : ""}`);
        return json({ ok: true, mailed, code });
      }
      if (c === "remove") {
        if (app.state !== "accepted") return fail("bad_request", 400);
        const reason = text(p.reason, 500);
        await run(env, "UPDATE applications SET state = 'removed', code = NULL, removed_reason = ?, decided = ? WHERE id = ?", reason, now, app.id);
        let mailed = false;
        if (p.send === true) mailed = await mail.send(env, app.email, mail.removed(app.route, app.nick, text(p.message, 2000)), { kind: "removed", app: app.id });
        await log("remove", app.id, `${app.nick}${reason ? ": " + reason : ""}`);
        return json({ ok: true, mailed });
      }
      if (c === "readmit") {
        if (app.state !== "removed") return fail("bad_request", 400);
        const code = randomId(24);
        await run(env, "UPDATE applications SET state = 'accepted', code = ?, removed_reason = '', decided = ? WHERE id = ?", code, now, app.id);
        let mailed = false;
        if (p.send === true) mailed = await mail.send(env, app.email, mail.newLink(app.route, app.nick, mail.pageLink(env, app.route, code)), { kind: "link", app: app.id });
        await log("readmit", app.id, app.nick);
        return json({ ok: true, mailed, code });
      }
      if (c === "newcode") {
        if (app.state !== "accepted") return fail("bad_request", 400);
        const code = randomId(24);
        await run(env, "UPDATE applications SET code = ? WHERE id = ?", code, app.id);
        let mailed = false;
        if (p.send === true) mailed = await mail.send(env, app.email, mail.newLink(app.route, app.nick, mail.pageLink(env, app.route, code)), { kind: "link", app: app.id });
        await log("new link", app.id, app.nick + (mailed ? ", e-mailed" : ""));
        return json({ ok: true, mailed, code, link: mail.pageLink(env, app.route, code) });
      }
      if (c === "rate") {
        const tags = [...new Set(str(p.tags).split(",").map((t) => line(t, 24).toLowerCase()).filter(Boolean))].slice(0, 8).join(",");
        await run(env, "UPDATE applications SET rating = ?, tags = ? WHERE id = ?", Math.max(0, Math.min(5, Math.round(Number(p.rating) || 0))), tags, app.id);
        return json({ ok: true });
      }
      if (c === "note") {
        await run(env, "UPDATE applications SET note = ? WHERE id = ?", text(p.note, 2000), app.id);
        return json({ ok: true });
      }
      if (c === "trial") {
        await run(env, "UPDATE applications SET trial_start = ? WHERE id = ?", p.start === false ? null : now, app.id);
        await log(p.start === false ? "trial cleared" : "trial started", app.id, app.nick);
        return json({ ok: true });
      }
      if (c === "delete") {
        await deleteApplication(env, app.id);
        await log("delete application", app.id, `${app.nick} <${app.email}>`);
        return json({ ok: true });
      }
      if (c === "delete-same-ip") {
        // spam: every application from the same address that is not accepted or removed
        if (!app.ip_hash) return json({ ok: true, deleted: 0 });
        const ids = (await all<{ id: string }>(env, "SELECT id FROM applications WHERE ip_hash = ? AND state NOT IN ('accepted', 'removed')", app.ip_hash)).map((r) => r.id);
        for (const id of ids) await deleteApplication(env, id);
        await log("delete same address", app.id, `${ids.length} applications`);
        return json({ ok: true, deleted: ids.length });
      }
      break;
    }
    case "mail": {
      if (b && c === "delete") {
        await run(env, "DELETE FROM mails WHERE id = ?", Number(b));
        return json({ ok: true });
      }
      if (b !== "send") break;
      const subject = line(p.subject, 160), bodyText = text(p.body, 10000);
      if (!subject || bodyText.length < 2) return fail("missing", 422, { field: !subject ? "subject" : "body" });
      if (!(await recipients(env, str(p.to))).length) return fail("no_recipients", 422);
      if (!env.RESEND_API_KEY) return fail("mail_not_configured", 503);
      const r = await sendToGroup(env, str(p.to), subject, bodyText, "message");
      await log("mail", str(p.to).startsWith("app:") ? str(p.to).slice(4) : str(p.to), `${subject} · ${r.sent} of ${r.total} sent`);
      return json({ ok: true, ...r });
    }
    case "template": {
      if (b && c === "delete") {
        await run(env, "DELETE FROM templates WHERE id = ?", b);
        return json({ ok: true });
      }
      const name = line(p.name, 60), subject = line(p.subject, 160), t = text(p.body, 10000);
      if (!name || !subject || !t) return fail("missing", 422);
      if (b) await run(env, "UPDATE templates SET name = ?, subject = ?, body = ? WHERE id = ?", name, subject, t, b);
      else await run(env, "INSERT INTO templates (id, name, subject, body, created) VALUES (?,?,?,?,?)", randomId(8), name, subject, t, now);
      return json({ ok: true });
    }
    case "settings": {
      const places = (v: unknown, d: number) => Math.max(0, Math.min(500, Math.round(Number(v ?? d)) || 0));
      const s = await settings(env);
      const flag = (v: unknown, old: string | undefined) => (v === undefined ? old ?? "1" : v ? "1" : "0");
      const next = {
        round: line(p.round ?? s.round, 40) || "Round 1",
        open: flag(p.open, s.open),
        closes: p.closes === undefined ? s.closes ?? "" : /^\d{4}-\d{2}-\d{2}$/.test(str(p.closes)) ? str(p.closes) : "",
        places_test: String(places(p.places_test, Number(s.places_test ?? 4))),
        places_licence: String(places(p.places_licence, Number(s.places_licence ?? 3))),
        pages_open: flag(p.pages_open, s.pages_open),
        apply_day: String(places(p.apply_day, Number(s.apply_day ?? 30)) || 1),
        image_days: String(Math.max(0, Math.min(3650, Math.round(Number(p.image_days ?? s.image_days ?? 30)) || 0))),
      };
      const extra: Record<string, string> = {};
      if (Array.isArray(p.push_events)) extra.push_events = p.push_events.map(str).filter((e) => e in EVENTS).join(",");
      for (const [k, v] of Object.entries({ ...next, ...extra })) await setSetting(env, k, v);
      await log("settings", "", Object.entries(next).map(([k, v]) => `${k}=${v}`).join(", "));
      return json({ ok: true, program: await program(env) });
    }
    case "roadmap": {
      if (b && c === "delete") {
        await run(env, "DELETE FROM votes WHERE item_id = ?", b);
        await run(env, "DELETE FROM roadmap WHERE id = ?", b);
        await log("delete roadmap card", b);
        return json({ ok: true });
      }
      if (b && c === "move") {
        // swap with the neighbour in the same column
        const items = await roadmapList(env);
        const me = items.find((i) => i.id === b);
        if (!me) return fail("not_found", 404);
        const col = items.filter((i) => i.col === me.col);
        const k = col.indexOf(me) + (Number(p.dir) < 0 ? -1 : 1);
        if (k < 0 || k >= col.length) return json({ ok: true });
        const order = col.map((i) => i.id);
        [order[k], order[col.indexOf(me)]] = [order[col.indexOf(me)]!, order[k]!];
        for (let i = 0; i < order.length; i++) await run(env, "UPDATE roadmap SET sort = ? WHERE id = ?", i * 10, order[i]);
        return json({ ok: true });
      }
      if (b && c === "reset-votes") {
        await run(env, "DELETE FROM votes WHERE item_id = ?", b);
        await log("reset votes", b);
        return json({ ok: true });
      }
      const title = line(p.title, 100);
      if (!title) return fail("missing", 422, { field: "title" });
      const fields = [pick(p.col, ["now", "next", "later", "ideas"], "ideas"), title, text(p.text, 300), pick(p.tier, ["free", "pro", "web"], "free"), Math.max(0, Math.round(Number(p.extra) || 0))];
      if (b) {
        await run(env, "UPDATE roadmap SET col = ?, title = ?, text = ?, tier = ?, extra = ? WHERE id = ?", ...fields, b);
        return json({ ok: true, id: b });
      }
      const id = title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 40) + "-" + randomId(4);
      const last = await one<{ s: number }>(env, "SELECT COALESCE(MAX(sort), 0) AS s FROM roadmap");
      await run(env, "INSERT INTO roadmap (id, col, title, text, tier, extra, sort, created) VALUES (?,?,?,?,?,?,?,?)", id, ...fields, (last?.s ?? 0) + 10, now);
      return json({ ok: true, id });
    }
    case "bug": {
      const bug = await one<{ id: number; title: string; state: string; reply: string; application_id: string }>(env, "SELECT id, title, state, reply, application_id FROM bugs WHERE id = ?", Number(b));
      if (!bug) return fail("not_found", 404);
      if (c === "delete") {
        await run(env, "DELETE FROM bugs WHERE id = ?", bug.id);
        await log("delete bug", bug.application_id, `#${bug.id} ${bug.title}`);
        return json({ ok: true });
      }
      if (c === "delete-image") {
        await run(env, "UPDATE bugs SET image = NULL, image_type = '' WHERE id = ?", bug.id);
        await log("delete screenshot", bug.application_id, `#${bug.id}`);
        return json({ ok: true });
      }
      const state = pick(p.state, BUG_STATES, bug.state);
      const reply = p.reply === undefined ? bug.reply : text(p.reply, 4000);
      // closed = when the report was closed (kept while it stays closed), replied = when the answer last changed
      await run(env, `UPDATE bugs SET state = ?1, note = ?2, reply = ?3,
          replied = CASE WHEN ?3 != reply THEN ?4 ELSE replied END,
          closed = CASE WHEN ?1 NOT IN ('fixed', 'cant-reproduce', 'duplicate') THEN NULL ELSE COALESCE(closed, ?4) END
        WHERE id = ?5`, state, text(p.note, 2000), reply, now, bug.id);
      let mailed = false;
      if (p.notify === true && reply) {
        const app = await one<App>(env, "SELECT id, route, nick, email, state, code, trial_start, ip_hash FROM applications WHERE id = ?", bug.application_id);
        if (app) mailed = await mail.send(env, app.email, mail.bugReply(app.nick, bug.id, bug.title, BUG_LABEL[state] ?? state, reply, mail.pageLink(env, app.route, app.code)), { kind: "bug-reply", app: app.id });
      }
      if (state !== bug.state) await log("bug state", bug.application_id, `#${bug.id}: ${state}`);
      return json({ ok: true, mailed });
    }
    case "feedback": {
      if (c !== "delete") break;
      await deleteFeedback(env, Number(b));
      await log("delete feedback", "", String(b));
      return json({ ok: true });
    }
    case "showcase": {
      if (b && c === "delete") {
        await run(env, "DELETE FROM showcase WHERE id = ?", b);
        await log("delete showcase", b);
        return json({ ok: true });
      }
      const game = line(p.game, 100), lang = line(p.lang, 60), link = line(p.link, 300), translator = line(p.translator, 80);
      if (!game || !lang || !translator || !isUrl(link)) return fail("missing", 422);
      const fields = [pick(p.state, ["pending", "approved", "rejected"], "pending"), game, lang, link, translator, text(p.blurb, 400), line(p.lines, 20), Math.round(Number(p.sort) || 0)];
      if (b) await run(env, "UPDATE showcase SET state = ?, game = ?, lang = ?, link = ?, translator = ?, blurb = ?, lines = ?, sort = ? WHERE id = ?", ...fields, b);
      else await run(env, "INSERT INTO showcase (state, game, lang, link, translator, blurb, lines, sort, id, created) VALUES (?,?,?,?,?,?,?,?,?,?)", ...fields, randomId(10), now);
      return json({ ok: true });
    }
    case "message": {
      const m = await one<{ id: number; name: string; email: string; body: string }>(env, "SELECT id, name, email, body FROM messages WHERE id = ?", Number(b));
      if (!m) return fail("not_found", 404);
      if (c === "reply") {
        const t = text(p.text, 8000);
        if (t.length < 2) return fail("missing", 422);
        const sent = isEmail(m.email) && (await mail.send(env, m.email, mail.reply(m.name, t, m.body), { kind: "reply" }));
        if (sent) await run(env, "UPDATE messages SET state = 'answered' WHERE id = ?", m.id);
        await log("reply", "", `${m.name}${sent ? "" : " (not sent)"}`);
        return json({ ok: true, sent });
      }
      if (c === "delete") {
        await run(env, "DELETE FROM messages WHERE id = ?", m.id);
        await log("delete message", "", m.name);
        return json({ ok: true });
      }
      await run(env, "UPDATE messages SET state = ? WHERE id = ?", pick(p.state, ["new", "answered", "archived"], "new"), m.id);
      return json({ ok: true });
    }
    case "release": {
      if (b && c === "delete") {
        await run(env, "DELETE FROM releases WHERE id = ?", b);
        await log("delete release", b);
        return json({ ok: true });
      }
      const version = line(p.version, 30);
      const date = /^\d{4}-\d{2}-\d{2}$/.test(str(p.date)) ? str(p.date) : new Date(now * 1000).toISOString().slice(0, 10);
      const dl = line(p.url, 400);
      if (!version) return fail("missing", 422, { field: "version" });
      if (dl && !/^https:\/\/\S+$/.test(dl)) return fail("missing", 422, { field: "url" });
      const sha = line(p.sha256, 64).toLowerCase();
      if (sha && !/^[0-9a-f]{64}$/.test(sha)) return fail("missing", 422, { field: "sha256" });
      const fields = [version, date, pick(p.channel, ["stable", "beta"], "stable"), line(p.title, 120), text(p.notes, 6000), dl, sha, line(p.size, 20), p.published ? 1 : 0];
      if (b) await run(env, "UPDATE releases SET version = ?, date = ?, channel = ?, title = ?, notes = ?, url = ?, sha256 = ?, size = ?, published = ? WHERE id = ?", ...fields, b);
      else await run(env, "INSERT INTO releases (version, date, channel, title, notes, url, sha256, size, published, id, created) VALUES (?,?,?,?,?,?,?,?,?,?,?)", ...fields, version.replace(/[^0-9a-z.]+/gi, "-").toLowerCase() + "-" + randomId(3), now);
      return json({ ok: true });
    }
    case "push": {
      if (b !== "test") break;
      const ok = await push(env, "health", "Aside website", "Test notification from the admin. It works!", { force: true });
      return json({ ok: true, sent: ok, configured: pushConfigured(env) });
    }
    case "status": {
      await setSetting(env, "status_on", p.on ? "1" : "0");
      await setSetting(env, "status_build", line(p.build, 60));
      await setSetting(env, "status_next", line(p.next, 80));
      await setSetting(env, "status_note", text(p.note, 400));
      await log("status", "", `${p.on ? "on" : "off"} · ${line(p.build, 60)}`);
      return json({ ok: true });
    }
    case "task": {
      if (b && c === "delete") {
        await run(env, "DELETE FROM task_results WHERE task_id = ?", b);
        await run(env, "DELETE FROM tasks WHERE id = ?", b);
        await log("delete task", b);
        return json({ ok: true });
      }
      const title = line(p.title, 140);
      if (!title) return fail("missing", 422, { field: "title" });
      const fields = [title, text(p.body, 3000), line(p.build, 40), p.open === false ? 0 : 1, Math.round(Number(p.sort) || 0)];
      const id = b ?? randomId(8);
      if (b) await run(env, "UPDATE tasks SET title = ?, body = ?, build = ?, open = ?, sort = ? WHERE id = ?", ...fields, b);
      else await run(env, "INSERT INTO tasks (title, body, build, open, sort, id, created) VALUES (?,?,?,?,?,?,?)", ...fields, id, now);
      let mailed: { sent: number; total: number } | null = null;
      if (p.notify === true)
        mailed = await sendToGroup(env, "test", `New testing task: ${title}`, `Hi {nick},\n\nthere is a new testing task for you:\n\n${title}${p.body ? "\n\n" + text(p.body, 3000) : ""}\n\nTick it off on your page when you have tried it:\n\n{link}`, "message");
      await log(b ? "edit task" : "new task", id, title + (mailed ? ` · e-mailed ${mailed.sent} of ${mailed.total}` : ""));
      return json({ ok: true, id, mailed });
    }
    case "build": {
      if (b && c === "delete") {
        await deleteBuild(env, b);
        await log("delete build", b);
        return json({ ok: true });
      }
      if (b && c === "part") {
        // one piece of the file: n from 0, base64 of at most 1 MB
        const bld = await one<{ id: string; size: number }>(env, "SELECT id, size FROM builds WHERE id = ? AND ready = 0", b);
        const data = str(p.data), n = Math.round(Number(p.n));
        if (!bld || !(n >= 0) || n > 200 || data.length > 1_400_000 || !/^[A-Za-z0-9+/]*={0,2}$/.test(data)) return fail("bad_request", 400);
        await run(env, "INSERT INTO build_parts (build_id, n, data) VALUES (?,?,?) ON CONFLICT(build_id, n) DO UPDATE SET data = excluded.data", b, n, data);
        await run(env, "UPDATE builds SET parts = (SELECT COUNT(*) FROM build_parts WHERE build_id = ?) WHERE id = ?", b, b);
        return json({ ok: true });
      }
      if (b && c === "finish") {
        // complete when the pieces are 0..parts-1 and add up to the size of the file
        const bld = await one<{ id: string; size: number; version: string }>(env, "SELECT id, size, version FROM builds WHERE id = ?", b);
        if (!bld) return fail("not_found", 404);
        const r = await one<{ n: number; mx: number; bytes: number }>(env, "SELECT COUNT(*) AS n, MAX(n) AS mx, SUM(length(data) * 3 / 4 - (CASE WHEN data LIKE '%==' THEN 2 WHEN data LIKE '%=' THEN 1 ELSE 0 END)) AS bytes FROM build_parts WHERE build_id = ?", b);
        if (!r || r.n === 0 || r.mx !== r.n - 1 || r.bytes !== bld.size) return fail("incomplete", 409, { parts: r?.n ?? 0, bytes: r?.bytes ?? 0 });
        await run(env, "UPDATE builds SET ready = 1, parts = ? WHERE id = ?", r.n, b);
        await log("new build", b, `${bld.version} · ${(bld.size / 1048576).toFixed(1)} MB`);
        return json({ ok: true });
      }
      if (b && c === "notify") {
        const bld = await one<{ version: string; notes: string; audience: string }>(env, "SELECT version, notes, audience FROM builds WHERE id = ? AND ready = 1", b);
        if (!bld) return fail("not_found", 404);
        if (!env.RESEND_API_KEY) return fail("mail_not_configured", 503);
        const to = bld.audience === "all" ? "accepted" : bld.audience;
        const r = await sendToGroup(env, to, line(p.subject, 160) || `Aside ${bld.version} is ready to test`, text(p.body, 8000) || `Hi {nick},\n\nAside ${bld.version} is ready.${bld.notes ? "\n\n" + bld.notes : ""}\n\nDownload it from your page:\n\n{link}`, "message");
        await log("build e-mail", b, `${bld.version} · ${r.sent} of ${r.total}`);
        return json({ ok: true, ...r });
      }
      const version = line(p.version, 40);
      if (!version) return fail("missing", 422, { field: "version" });
      const audience = pick(p.audience, ["test", "licence", "all"], "test");
      if (b) {
        await run(env, "UPDATE builds SET version = ?, notes = ?, audience = ?, visible = ? WHERE id = ?", version, text(p.notes, 4000), audience, p.visible === false ? 0 : 1, b);
        return json({ ok: true, id: b });
      }
      const url = line(p.url, 500);
      if (url && !/^https:\/\/\S+$/.test(url)) return fail("missing", 422, { field: "url" });
      const size = Math.round(Number(p.size) || 0);
      if (!url && (size <= 0 || size > 60 * 1024 * 1024)) return fail("too_big", 413);
      const sha = line(p.sha256, 64).toLowerCase();
      const id = slug(version) + "-" + randomId(4);
      await run(env, "INSERT INTO builds (id, created, version, notes, audience, url, filename, size, sha256, ready, visible) VALUES (?,?,?,?,?,?,?,?,?,?,?)",
        id, now, version, text(p.notes, 4000), audience, url, line(p.filename, 120), url ? 0 : size, /^[0-9a-f]{64}$/.test(sha) ? sha : "", url ? 1 : 0, p.visible === false ? 0 : 1);
      if (url) await log("new build", id, `${version} · link`);
      return json({ ok: true, id });
    }
    case "subscriber": {
      if (b && c === "delete") {
        await run(env, "DELETE FROM subscribers WHERE id = ?", Number(b));
        return json({ ok: true });
      }
      break;
    }
    case "newsletter": {
      if (b !== "send") break;
      const subject = line(p.subject, 160), bodyText = text(p.body, 20000);
      if (!subject || bodyText.length < 2) return fail("missing", 422);
      if (!env.RESEND_API_KEY) return fail("mail_not_configured", 503);
      const topic = TOPICS.includes(str(p.topic)) ? str(p.topic) : "";
      // a test goes only to you, with a dummy unsubscribe link
      if (p.test === true) {
        const ok = await mail.send(env, env.OWNER_EMAIL ?? "aside@filiposakkq.dev", mail.newsletter("[Test] " + subject, bodyText, site(env) + "/news.html#manage=test"));
        return json({ ok: true, sent: ok ? 1 : 0, total: 1 });
      }
      const subs = await all<{ email: string; unsub: string }>(env, "SELECT email, unsub FROM subscribers WHERE confirmed IS NOT NULL AND (? = '' OR (',' || topics || ',') LIKE '%,' || ? || ',%')", topic, topic);
      if (!subs.length) return fail("no_recipients", 422);
      const sent = await mail.sendMany(env, subs.map((x) => {
        const manage = `${site(env)}/news.html#manage=${x.unsub}`;
        return { to: x.email, mail: mail.newsletter(subject, bodyText, manage), headers: { "List-Unsubscribe": `<${site(env)}/api/unsubscribe/${x.unsub}>`, "List-Unsubscribe-Post": "List-Unsubscribe=One-Click" } };
      }));
      await run(env, "INSERT INTO newsletters (created, topic, subject, body, total, sent) VALUES (?,?,?,?,?,?)", now, topic || "all", subject, bodyText, subs.length, sent);
      await log("newsletter", "", `${subject} · ${sent} of ${subs.length}`);
      return json({ ok: true, sent, total: subs.length });
    }
    case "faq": {
      if (b && c === "delete") {
        await run(env, "DELETE FROM faq WHERE id = ?", b);
        return json({ ok: true });
      }
      const q2 = line(p.q, 200), a2 = text(p.a, 3000);
      if (!q2 || !a2) return fail("missing", 422);
      const fields = [q2, a2, Math.round(Number(p.sort) || 0), p.published === false ? 0 : 1];
      if (b) await run(env, "UPDATE faq SET q = ?, a = ?, sort = ?, published = ? WHERE id = ?", ...fields, b);
      else await run(env, "INSERT INTO faq (q, a, sort, published, id) VALUES (?,?,?,?,?)", ...fields, (slug(q2) || "q") + "-" + randomId(3));
      return json({ ok: true });
    }
    case "post": {
      if (b && c === "delete") {
        await run(env, "DELETE FROM posts WHERE id = ?", b);
        await log("delete post", b);
        return json({ ok: true });
      }
      const title = line(p.title, 140), bodyText = text(p.body, 60000);
      if (!title || bodyText.length < 2) return fail("missing", 422, { field: !title ? "title" : "body" });
      const date = /^\d{4}-\d{2}-\d{2}$/.test(str(p.date)) ? str(p.date) : new Date(now * 1000).toISOString().slice(0, 10);
      const fields = [date, title, line(p.dek, 240), line(p.tag, 40), bodyText, p.published ? 1 : 0];
      if (b) {
        await run(env, "UPDATE posts SET date = ?, title = ?, dek = ?, tag = ?, body = ?, published = ? WHERE id = ?", ...fields, b);
        return json({ ok: true, id: b });
      }
      let id = slug(title) || "post";
      if (await one(env, "SELECT 1 FROM posts WHERE id = ?", id)) id += "-" + randomId(3);
      await run(env, "INSERT INTO posts (date, title, dek, tag, body, published, id, created) VALUES (?,?,?,?,?,?,?,?)", ...fields, id, now);
      await log("new post", id, title);
      return json({ ok: true, id });
    }
    case "bulk": {
      // delete many rows of one kind at once
      const kind = KINDS[str(p.kind)];
      const ids = (Array.isArray(p.ids) ? p.ids : []).slice(0, 500).map((x) => (typeof x === "number" ? x : str(x))).filter((x) => x !== "");
      if (!kind || !ids.length) return fail("bad_request", 400);
      for (const id of ids) {
        if (str(p.kind) === "applications") await deleteApplication(env, String(id));
        else if (str(p.kind) === "feedback") await deleteFeedback(env, Number(id));
        else if (str(p.kind) === "builds") await deleteBuild(env, String(id));
        else if (str(p.kind) === "tasks") {
          await run(env, "DELETE FROM task_results WHERE task_id = ?", String(id));
          await run(env, "DELETE FROM tasks WHERE id = ?", String(id));
        }
        else await run(env, `DELETE FROM ${kind.table} WHERE id = ?`, kind.key === "num" ? Number(id) : String(id));
      }
      // deleting log entries leaves no new entry behind (else the log is never empty)
      if (str(p.kind) !== "log") await log("bulk delete", "", `${ids.length} × ${str(p.kind)}`);
      return json({ ok: true, deleted: ids.length });
    }
    case "cleanup": {
      const days = Math.max(0, Math.min(3650, Math.round(Number(p.days) || 0)));
      const before = now - days * DAY;
      let n = 0;
      switch (str(p.what)) {
        case "declined": {
          const ids = (await all<{ id: string }>(env, "SELECT id FROM applications WHERE state = 'declined' AND COALESCE(decided, created) <= ?", before)).map((r) => r.id);
          for (const id of ids) await deleteApplication(env, id);
          n = ids.length;
          break;
        }
        case "unconfirmed": {
          const ids = (await all<{ id: string }>(env, "SELECT id FROM applications WHERE verified IS NULL AND created <= ?", before)).map((r) => r.id);
          for (const id of ids) await deleteApplication(env, id);
          n = ids.length;
          break;
        }
        case "images":
          n = (await run(env, "UPDATE bugs SET image = NULL, image_type = '' WHERE image IS NOT NULL AND state IN ('fixed', 'cant-reproduce', 'duplicate') AND COALESCE(closed, created) <= ?", before)).meta.changes ?? 0;
          break;
        case "all-images":
          n = (await run(env, "UPDATE bugs SET image = NULL, image_type = '' WHERE image IS NOT NULL AND created <= ?", before)).meta.changes ?? 0;
          break;
        case "mails":
          n = (await run(env, "DELETE FROM mails WHERE created <= ?", before)).meta.changes ?? 0;
          break;
        case "messages":
          n = (await run(env, "DELETE FROM messages WHERE state = 'archived' AND created <= ?", before)).meta.changes ?? 0;
          break;
        case "log":
          n = (await run(env, "DELETE FROM log WHERE at <= ?", before)).meta.changes ?? 0;
          break;
        case "votes":
          n = (await run(env, "DELETE FROM votes")).meta.changes ?? 0;
          break;
        default:
          return fail("bad_request", 400);
      }
      if (str(p.what) !== "log") await log("clean up", "", `${str(p.what)}${days ? ` older than ${days} d` : ""}: ${n}`);
      return json({ ok: true, deleted: n });
    }
  }
  return fail("not_found", 404);
}
