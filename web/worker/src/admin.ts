import type { Env } from "./env.ts";
import * as mail from "./mail.ts";
import { program, roadmapList, settings } from "./public.ts";
import { body, fail, isUrl, json, line, randomId, safeEqual, str, text } from "./util.ts";

function authorised(req: Request, env: Env): boolean {
  const given = (req.headers.get("authorization") ?? "").replace(/^Bearer\s+/i, "");
  return !!env.ADMIN_TOKEN && env.ADMIN_TOKEN.length >= 16 && safeEqual(given, env.ADMIN_TOKEN);
}

const all = async <T = Record<string, unknown>>(env: Env, sql: string, ...args: unknown[]) => (await env.DB.prepare(sql).bind(...args).all<T>()).results;
const one = <T = Record<string, unknown>>(env: Env, sql: string, ...args: unknown[]) => env.DB.prepare(sql).bind(...args).first<T>();
const run = (env: Env, sql: string, ...args: unknown[]) => env.DB.prepare(sql).bind(...args).run();
const pick = (v: unknown, allowed: string[], fallback: string) => (allowed.includes(str(v)) ? str(v) : fallback);

interface App { id: string; route: "test" | "licence"; nick: string; email: string; state: string; code: string | null; trial_start: number | null }

/** `/admin/api/...`: every call needs `Authorization: Bearer <ADMIN_TOKEN>`. */
export async function adminApi(req: Request, env: Env, now: number, path: string): Promise<Response> {
  if (!env.ADMIN_TOKEN || env.ADMIN_TOKEN.length < 16) return fail("admin_not_configured", 503);
  if (!authorised(req, env)) return fail("unauthorised", 401);
  const url = new URL(req.url);
  const [a, b, c] = path.split("/").filter(Boolean);
  const q = (k: string) => url.searchParams.get(k) ?? "";

  if (req.method === "GET") {
    switch (a) {
      case "overview": {
        const count = async (sql: string) => (await one<{ n: number }>(env, sql))?.n ?? 0;
        return json({
          ok: true,
          program: await program(env),
          mail: !!env.RESEND_API_KEY,
          turnstile: !!env.TURNSTILE_SECRET,
          counts: {
            applications: await count("SELECT COUNT(*) AS n FROM applications WHERE state = 'new'"),
            feedback_due: await count("SELECT COUNT(*) AS n FROM applications WHERE route = 'licence' AND state = 'accepted' AND ((fb2_sent IS NOT NULL AND fb2_done IS NULL) OR (fb4_sent IS NOT NULL AND fb4_done IS NULL))"),
            ready: await count("SELECT COUNT(*) AS n FROM applications WHERE route = 'licence' AND state = 'accepted' AND fb4_done IS NOT NULL"),
            bugs: await count("SELECT COUNT(*) AS n FROM bugs WHERE state = 'new'"),
            messages: await count("SELECT COUNT(*) AS n FROM messages WHERE state = 'new'"),
            showcase: await count("SELECT COUNT(*) AS n FROM showcase WHERE state = 'pending'"),
            votes: await count("SELECT COUNT(*) AS n FROM votes"),
            downloads: await count("SELECT COALESCE(SUM(downloads), 0) AS n FROM releases"),
          },
          recent: await all(env, `SELECT * FROM (
              SELECT 'application' AS kind, id, created, nick AS who, route AS detail FROM applications
              UNION ALL SELECT 'bug', CAST(id AS TEXT), created, title, severity FROM bugs
              UNION ALL SELECT 'message', CAST(id AS TEXT), created, name, topic FROM messages
              UNION ALL SELECT 'showcase', id, created, translator, game FROM showcase WHERE created > 0
              UNION ALL SELECT 'feedback', CAST(f.id AS TEXT), f.created, a.nick, f.which FROM feedback f JOIN applications a ON a.id = f.application_id
            ) ORDER BY created DESC LIMIT 15`),
        });
      }
      case "applications":
        return json({
          ok: true,
          items: await all(env, `SELECT id, route, created, nick, email, lang, state, trial_start, fb2_sent, fb2_done, fb4_sent, fb4_done FROM applications
            WHERE (? = '' OR route = ?) AND (? = '' OR state = ?) ORDER BY CASE state WHEN 'new' THEN 0 WHEN 'waitlist' THEN 1 WHEN 'accepted' THEN 2 ELSE 3 END, created DESC LIMIT 300`, q("route"), q("route"), q("state"), q("state")),
        });
      case "application": {
        const app = await one(env, "SELECT * FROM applications WHERE id = ?", b);
        if (!app) return fail("not_found", 404);
        delete (app as Record<string, unknown>).ip_hash;
        return json({ ok: true, item: app, feedback: await all(env, "SELECT * FROM feedback WHERE application_id = ? ORDER BY created", b), bugs: await all(env, "SELECT id, created, title, state FROM bugs WHERE application_id = ? ORDER BY id DESC", b) });
      }
      case "settings":
        return json({ ok: true, settings: await settings(env), program: await program(env) });
      case "roadmap":
        return json({ ok: true, items: await all(env, "SELECT r.*, (SELECT COUNT(*) FROM votes v WHERE v.item_id = r.id) AS live FROM roadmap r ORDER BY r.sort, r.created") });
      case "bugs":
        return json({ ok: true, items: await all(env, "SELECT b.id, b.created, b.title, b.steps, b.expected, b.actual, b.version, b.severity, b.state, b.note, b.image_type, a.nick, a.email FROM bugs b JOIN applications a ON a.id = b.application_id WHERE (? = '' OR b.state = ?) ORDER BY b.id DESC LIMIT 300", q("state"), q("state")) });
      case "bug": {
        if (c !== "image") break;
        const r = await one<{ image: ArrayBuffer | null; image_type: string }>(env, "SELECT image, image_type FROM bugs WHERE id = ?", Number(b));
        if (!r?.image) return fail("not_found", 404);
        return new Response(r.image, { headers: { "content-type": r.image_type, "cache-control": "private, max-age=3600", "content-security-policy": "default-src 'none'" } });
      }
      case "showcase":
        return json({ ok: true, items: await all(env, "SELECT id, created, state, game, lang, link, translator, email, blurb, lines, sort FROM showcase ORDER BY CASE state WHEN 'pending' THEN 0 WHEN 'approved' THEN 1 ELSE 2 END, sort, created DESC") });
      case "messages":
        return json({ ok: true, items: await all(env, "SELECT id, created, name, email, topic, body, state FROM messages WHERE (? = '' OR state = ?) ORDER BY id DESC LIMIT 300", q("state"), q("state")) });
      case "releases":
        return json({ ok: true, items: await all(env, "SELECT * FROM releases ORDER BY date DESC, created DESC") });
      case "feedback":
        return json({ ok: true, items: await all(env, "SELECT f.*, a.nick, a.email FROM feedback f JOIN applications a ON a.id = f.application_id ORDER BY f.created DESC LIMIT 200") });
    }
    return fail("not_found", 404);
  }

  if (req.method !== "POST") return fail("not_found", 404);
  const p = (await body(req, 200_000)) ?? {};

  switch (a) {
    case "application": {
      const app = await one<App>(env, "SELECT id, route, nick, email, state, code, trial_start FROM applications WHERE id = ?", b);
      if (!app) return fail("not_found", 404);
      if (c === "decide") {
        const state = pick(p.state, ["accepted", "waitlist", "declined", "new"], "");
        if (!state) return fail("bad_request", 400);
        const code = state === "accepted" ? app.code ?? randomId(24) : app.code;
        const trial = state === "accepted" && app.route === "licence" && p.start_trial !== false ? app.trial_start ?? now : app.trial_start;
        await run(env, "UPDATE applications SET state = ?, decided = ?, code = ?, trial_start = ? WHERE id = ?", state, now, code, trial, app.id);
        let mailed = false;
        if (p.send === true && state !== "new") mailed = await mail.send(env, app.email, mail.decision(app.route, state as "accepted", app.nick, code ?? "", env, text(p.message, 2000)));
        return json({ ok: true, mailed, code });
      }
      if (c === "note") {
        await run(env, "UPDATE applications SET note = ? WHERE id = ?", text(p.note, 2000), app.id);
        return json({ ok: true });
      }
      if (c === "trial") {
        await run(env, "UPDATE applications SET trial_start = ? WHERE id = ?", p.start === false ? null : now, app.id);
        return json({ ok: true });
      }
      if (c === "delete") {
        // on request of the applicant: the application, its feedback and its bug reports go
        await run(env, "DELETE FROM feedback WHERE application_id = ?", app.id);
        await run(env, "DELETE FROM bugs WHERE application_id = ?", app.id);
        await run(env, "DELETE FROM applications WHERE id = ?", app.id);
        return json({ ok: true });
      }
      break;
    }
    case "settings": {
      const places = (v: unknown, d: number) => Math.max(0, Math.min(500, Math.round(Number(v ?? d)) || 0));
      const s = await settings(env);
      const next = {
        round: line(p.round ?? s.round, 40) || "Round 1",
        open: p.open === undefined ? s.open ?? "1" : p.open ? "1" : "0",
        closes: /^\d{4}-\d{2}-\d{2}$/.test(str(p.closes)) ? str(p.closes) : "",
        places_test: String(places(p.places_test, Number(s.places_test ?? 4))),
        places_licence: String(places(p.places_licence, Number(s.places_licence ?? 3))),
      };
      for (const [k, v] of Object.entries(next)) await run(env, "INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value", k, v);
      return json({ ok: true, program: await program(env) });
    }
    case "roadmap": {
      if (b && c === "delete") {
        await run(env, "DELETE FROM votes WHERE item_id = ?", b);
        await run(env, "DELETE FROM roadmap WHERE id = ?", b);
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
      await run(env, "UPDATE bugs SET state = ?, note = ? WHERE id = ?", pick(p.state, ["new", "confirmed", "fixed", "cant-reproduce", "duplicate"], "new"), text(p.note, 2000), Number(b));
      return json({ ok: true });
    }
    case "showcase": {
      if (b && c === "delete") {
        await run(env, "DELETE FROM showcase WHERE id = ?", b);
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
        const sent = await mail.send(env, m.email, mail.reply(m.name, t, m.body));
        if (sent) await run(env, "UPDATE messages SET state = 'answered' WHERE id = ?", m.id);
        return json({ ok: true, sent });
      }
      if (c === "delete") {
        await run(env, "DELETE FROM messages WHERE id = ?", m.id);
        return json({ ok: true });
      }
      await run(env, "UPDATE messages SET state = ? WHERE id = ?", pick(p.state, ["new", "answered", "archived"], "new"), m.id);
      return json({ ok: true });
    }
    case "release": {
      if (b && c === "delete") {
        await run(env, "DELETE FROM releases WHERE id = ?", b);
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
  }
  return fail("not_found", 404);
}
