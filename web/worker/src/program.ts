import type { Env } from "./env.ts";
import * as mail from "./mail.ts";
import { push } from "./notify.ts";
import { type AppRow, byCode, settings } from "./public.ts";
import { body, fail, hit, human, ipHash, isEmail, json, line, over, randomId, str, text, within } from "./util.ts";

const DAY = 86400;

/** Base64 to bytes: the runtime's own decoder where it has one (fast, little CPU), else atob. */
const fromBase64 = (b64: string): Uint8Array => {
  const native = (Uint8Array as unknown as { fromBase64?: (s: string) => Uint8Array }).fromBase64;
  if (native) return native(b64);
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
};

// ---------------------------------------------------------------- testing tasks and builds on the personal pages

export async function tasksFor(env: Env, appId: string) {
  return (
    await env.DB.prepare(
      "SELECT t.id, t.title, t.body, t.build, r.state, r.note FROM tasks t LEFT JOIN task_results r ON r.task_id = t.id AND r.application_id = ? WHERE t.open = 1 ORDER BY t.sort, t.created DESC",
    ).bind(appId).all()
  ).results;
}

export async function buildsFor(env: Env, a: AppRow) {
  return (
    await env.DB.prepare("SELECT id, created, version, notes, filename, size, sha256, url != '' AS link FROM builds WHERE ready = 1 AND visible = 1 AND (audience = 'all' OR audience = ?) ORDER BY created DESC LIMIT 10")
      .bind(a.route)
      .all()
  ).results;
}

/** A tester ticks off a task: done, problem or skipped, with a short note. */
export async function taskResult(req: Request, env: Env, now: number): Promise<Response> {
  const b = await body(req, 10_000);
  if (!b) return fail("bad_request", 400);
  const a = await byCode(req, env, str(b.code), now);
  if (a instanceof Response) return a;
  const task = await env.DB.prepare("SELECT id, title FROM tasks WHERE id = ? AND open = 1").bind(str(b.task)).first<{ id: string; title: string }>();
  if (!task) return fail("not_found", 404);
  const state = ["done", "problem", "skipped"].includes(str(b.state)) ? str(b.state) : "";
  if (!state) return fail("bad_request", 400);
  if (!(await within(env, "code:" + a.id, "task", 60, DAY, now))) return fail("too_many", 429);
  await env.DB.prepare("INSERT INTO task_results (task_id, application_id, state, note, at) VALUES (?,?,?,?,?) ON CONFLICT(task_id, application_id) DO UPDATE SET state = excluded.state, note = excluded.note, at = excluded.at")
    .bind(task.id, a.id, state, text(b.note, 1500), now)
    .run();
  if (state === "problem") await push(env, "bugs", `Task problem · ${task.title}`, `${a.nick}: ${text(b.note, 300) || "(no note)"}`);
  return json({ ok: true });
}

/** A build through a personal link: the outside link (redirect) or the file put together from its pieces. Each download is counted. */
export async function buildDownload(req: Request, env: Env, id: string, code: string, now: number): Promise<Response> {
  const a = await byCode(req, env, code, now);
  if (a instanceof Response) return a;
  const bld = await env.DB.prepare("SELECT id, url, filename, size, parts FROM builds WHERE id = ? AND ready = 1 AND visible = 1 AND (audience = 'all' OR audience = ?)")
    .bind(id, a.route)
    .first<{ id: string; url: string; filename: string; size: number; parts: number }>();
  if (!bld) return fail("not_found", 404);
  await env.DB.prepare("INSERT INTO build_downloads (build_id, application_id, at) VALUES (?,?,?)").bind(bld.id, a.id, now).run();
  if (bld.url) return Response.redirect(bld.url, 302);
  const db = env.DB;
  let n = 0;
  const stream = new ReadableStream<Uint8Array>({
    async pull(ctrl) {
      if (n >= bld.parts) return ctrl.close();
      const part = await db.prepare("SELECT data FROM build_parts WHERE build_id = ? AND n = ?").bind(bld.id, n++).first<{ data: string }>();
      if (!part) return ctrl.error(new Error("missing part"));
      ctrl.enqueue(fromBase64(part.data));
    },
  });
  return new Response(stream, {
    headers: {
      "content-type": "application/octet-stream",
      "content-length": String(bld.size),
      "content-disposition": `attachment; filename="${bld.filename.replace(/[^\w.\- ]+/g, "_") || "aside-build"}"`,
      "cache-control": "no-store",
    },
  });
}

// ---------------------------------------------------------------- pages edited in the admin: status, FAQ, blog

export async function status(env: Env): Promise<Response> {
  const s = await settings(env);
  if (s.status_on !== "1") return json({ ok: true, on: false }, 200, { "cache-control": "public, max-age=120" });
  const n = async (sql: string) => (await env.DB.prepare(sql).first<{ n: number }>())?.n ?? 0;
  return json(
    {
      ok: true,
      on: true,
      build: s.status_build ?? "",
      next: s.status_next ?? "",
      note: s.status_note ?? "",
      numbers: {
        testers: await n("SELECT COUNT(*) AS n FROM applications WHERE state = 'accepted' AND route = 'test'"),
        fixed: await n("SELECT COUNT(*) AS n FROM bugs WHERE state = 'fixed'"),
        open: await n("SELECT COUNT(*) AS n FROM bugs WHERE state IN ('new', 'confirmed')"),
      },
    },
    200,
    { "cache-control": "public, max-age=120" },
  );
}

export async function faq(env: Env): Promise<Response> {
  const items = (await env.DB.prepare("SELECT id, q, a FROM faq WHERE published = 1 ORDER BY sort, id").all()).results;
  return json({ ok: true, items }, 200, { "cache-control": "public, max-age=120" });
}

export async function posts(env: Env): Promise<Response> {
  const items = (await env.DB.prepare("SELECT id, date, title, dek, tag, body FROM posts WHERE published = 1 ORDER BY date DESC, created DESC LIMIT 50").all()).results;
  return json({ ok: true, items }, 200, { "cache-control": "public, max-age=120" });
}

// ---------------------------------------------------------------- the newsletter

export const TOPICS = ["releases", "rounds", "diary"];
const topicsOf = (v: unknown) => {
  const t = (Array.isArray(v) ? v : []).map(str).filter((x) => TOPICS.includes(x));
  return [...new Set(t)].join(",");
};
const TOKEN_RE = /^[a-z2-9]{16,40}$/;

/** Asks for the newsletter. The answer is the same whether the address is new or known, so nobody can test who subscribed. */
export async function subscribe(req: Request, env: Env, now: number): Promise<Response> {
  const b = await body(req, 5000);
  if (!b) return fail("bad_request", 400);
  if (str(b.website)) return json({ ok: true });
  const email = line(b.email, 200).toLowerCase();
  if (!isEmail(email)) return fail("missing", 422, { field: "email" });
  const topics = topicsOf(b.topics);
  if (!topics) return fail("missing", 422, { field: "topics" });
  if (!(await human(env, b.turnstile, req))) return fail("not_human", 403);
  const ip = await ipHash(req, env);
  if (!(await within(env, ip, "subscribe", 3, DAY, now))) return fail("too_many", 429);
  const old = await env.DB.prepare("SELECT id, confirmed, created FROM subscribers WHERE email = ?").bind(email).first<{ id: number; confirmed: number | null; created: number }>();
  if (old?.confirmed) return json({ ok: true, confirm: true }); // already in: nothing to send
  if (old && now - old.created < 2 * DAY) return json({ ok: true, confirm: true }); // a confirmation is already on its way
  const token = env.RESEND_API_KEY ? randomId(24) : null;
  if (old) await env.DB.prepare("UPDATE subscribers SET created = ?, topics = ?, token = ?, confirmed = ?, ip_hash = ? WHERE id = ?").bind(now, topics, token, token ? null : now, ip, old.id).run();
  else await env.DB.prepare("INSERT INTO subscribers (created, email, topics, token, confirmed, unsub, ip_hash) VALUES (?,?,?,?,?,?,?)").bind(now, email, topics, token, token ? null : now, randomId(24), ip).run();
  if (token) await mail.send(env, email, mail.subscribeConfirm(token, env), { kind: "news-confirm" });
  return json({ ok: true, confirm: !!token });
}

export async function subscribeConfirm(req: Request, env: Env, now: number): Promise<Response> {
  const b = await body(req, 2000);
  const token = str(b?.token);
  const ip = await ipHash(req, env);
  if (await over(env, ip, "badcode", 20, 3600, now)) return fail("too_many", 429);
  const s = TOKEN_RE.test(token)
    ? await env.DB.prepare("SELECT id, email, unsub FROM subscribers WHERE token = ? AND confirmed IS NULL AND created > ?").bind(token, now - 2 * DAY).first<{ id: number; email: string; unsub: string }>()
    : null;
  if (!s) {
    await hit(env, ip, "badcode", now);
    return fail("not_found", 404);
  }
  await env.DB.prepare("UPDATE subscribers SET confirmed = ?, token = NULL WHERE id = ?").bind(now, s.id).run();
  await push(env, "subscribers", "New newsletter subscriber", s.email);
  return json({ ok: true, manage: s.unsub });
}

/** The page behind the link at the bottom of every newsletter: see and change topics, or unsubscribe. */
export async function subscription(req: Request, env: Env, token: string, now: number): Promise<Response> {
  const ip = await ipHash(req, env);
  if (await over(env, ip, "badcode", 20, 3600, now)) return fail("too_many", 429);
  const s = TOKEN_RE.test(token) ? await env.DB.prepare("SELECT id, email, topics, confirmed FROM subscribers WHERE unsub = ?").bind(token).first<{ id: number; email: string; topics: string; confirmed: number | null }>() : null;
  if (!s) {
    await hit(env, ip, "badcode", now);
    return fail("not_found", 404);
  }
  if (req.method === "GET") return json({ ok: true, email: s.email.replace(/^(.).*(@.*)$/, "$1…$2"), topics: s.topics.split(",").filter(Boolean) });
  const b = await body(req, 2000);
  if (b?.unsubscribe === true) {
    await env.DB.prepare("DELETE FROM subscribers WHERE id = ?").bind(s.id).run();
    return json({ ok: true, gone: true });
  }
  const topics = topicsOf(b?.topics);
  if (!topics) {
    await env.DB.prepare("DELETE FROM subscribers WHERE id = ?").bind(s.id).run();
    return json({ ok: true, gone: true });
  }
  await env.DB.prepare("UPDATE subscribers SET topics = ? WHERE id = ?").bind(topics, s.id).run();
  return json({ ok: true, topics: topics.split(",") });
}

/** The one-click unsubscribe of mail programs (List-Unsubscribe-Post, RFC 8058): POST /api/unsubscribe/<token>. */
export async function oneClick(env: Env, token: string): Promise<Response> {
  if (TOKEN_RE.test(token)) await env.DB.prepare("DELETE FROM subscribers WHERE unsub = ?").bind(token).run();
  return json({ ok: true });
}
