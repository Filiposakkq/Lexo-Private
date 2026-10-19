import type { Env } from "./env.ts";
import * as mail from "./mail.ts";
import { push, pushConfigured } from "./notify.ts";

const DAY = 86400;

interface Due { id: string; nick: string; email: string; code: string; trial_start: number; fb2_sent: number | null; fb2_reminded: number | null; fb2_done: number | null; fb4_sent: number | null; fb4_reminded: number | null; fb4_done: number | null }

/** Once a day: the feedback forms of sponsored licences (after 14 and 28 days, a reminder 4 days later if not answered), and the clean-up:
 *  flood counters after 2 days, unconfirmed applications after 48 hours, screenshots of closed bug reports after `image_days` (0 = never). */
export async function daily(env: Env, now: number): Promise<{ sent: number }> {
  let sent = 0;
  const rows = (await env.DB.prepare("SELECT * FROM applications WHERE route = 'licence' AND state = 'accepted' AND trial_start IS NOT NULL AND code IS NOT NULL AND fb4_done IS NULL").all<Due>()).results;
  for (const a of rows) {
    const age = now - a.trial_start;
    const step = async (which: "2w" | "4w", col: "fb2" | "fb4") => {
      if (a[`${col}_done`]) return;
      if (!a[`${col}_sent`]) {
        await mail.send(env, a.email, mail.feedbackInvite(which, a.nick, a.code, env, false), { kind: "feedback", app: a.id });
        await env.DB.prepare(`UPDATE applications SET ${col}_sent = ? WHERE id = ?`).bind(now, a.id).run();
        sent++;
      } else if (!a[`${col}_reminded`] && now - (a[`${col}_sent`] ?? now) >= 4 * DAY) {
        await mail.send(env, a.email, mail.feedbackInvite(which, a.nick, a.code, env, true), { kind: "reminder", app: a.id });
        await env.DB.prepare(`UPDATE applications SET ${col}_reminded = ? WHERE id = ?`).bind(now, a.id).run();
        sent++;
      }
    };
    if (age >= 14 * DAY) await step("2w", "fb2");
    if (age >= 28 * DAY && a.fb2_done) await step("4w", "fb4");
  }
  await env.DB.prepare("DELETE FROM hits WHERE at < ?").bind(now - 2 * DAY).run();
  const stale = (await env.DB.prepare("SELECT id FROM applications WHERE verified IS NULL AND created < ?").bind(now - 2 * DAY).all<{ id: string }>()).results;
  for (const s of stale) {
    await env.DB.prepare("DELETE FROM mails WHERE application_id = ?").bind(s.id).run();
    await env.DB.prepare("DELETE FROM applications WHERE id = ?").bind(s.id).run();
  }
  const days = Number((await env.DB.prepare("SELECT value FROM settings WHERE key = 'image_days'").first<{ value: string }>())?.value ?? 30);
  let images = 0;
  if (days > 0)
    images = (await env.DB.prepare("UPDATE bugs SET image = NULL, image_type = '' WHERE image IS NOT NULL AND closed IS NOT NULL AND closed < ?").bind(now - days * DAY).run()).meta.changes ?? 0;
  // newsletter sign-ups that were never confirmed
  await env.DB.prepare("DELETE FROM subscribers WHERE confirmed IS NULL AND created < ?").bind(now - 2 * DAY).run();
  await env.DB.prepare("INSERT INTO settings (key, value) VALUES ('last_daily', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value").bind(String(now)).run();
  if (sent || stale.length || images)
    await env.DB.prepare("INSERT INTO log (at, who, action, detail) VALUES (?, 'system', 'daily', ?)").bind(now, `${sent} feedback e-mails, ${stale.length} unconfirmed removed, ${images} screenshots removed`).run();
  return { sent };
}

export interface Check { key: string; label: string; ok: boolean | null; detail: string }

/** What can be checked from inside: e-mail, the sending domain, the database, the daily job, and the keys. ok null = cannot tell. */
export async function health(env: Env, now: number): Promise<Check[]> {
  const out: Check[] = [];
  const setting = async (k: string) => (await env.DB.prepare("SELECT value FROM settings WHERE key = ?").bind(k).first<{ value: string }>())?.value ?? "";
  // the database answers and has room (D1 free plan: 500 MB)
  try {
    const size = (await env.DB.prepare("SELECT 1").run()).meta.size_after ?? 0;
    out.push({ key: "database", label: "Database", ok: size < 400 * 1024 * 1024, detail: `${(size / 1048576).toFixed(1)} MB of 500 MB` });
  } catch (e) {
    out.push({ key: "database", label: "Database", ok: false, detail: "does not answer" });
    return out;
  }
  // e-mail: set up, and not failing again and again
  if (!env.RESEND_API_KEY) out.push({ key: "mail", label: "E-mail", ok: false, detail: "RESEND_API_KEY is not set: nothing is mailed" });
  else {
    // failing = the last three e-mails all failed, the newest of them in the last 6 hours
    const last = (await env.DB.prepare("SELECT sent, created FROM mails ORDER BY id DESC LIMIT 3").all<{ sent: number; created: number }>()).results;
    const failing = last.length === 3 && last.every((m) => !m.sent) && now - last[0]!.created < 6 * 3600;
    out.push({ key: "mail", label: "E-mail", ok: !failing, detail: failing ? "the last 3 e-mails failed" : last.length ? `last e-mail ${last[0]!.sent ? "sent" : "failed"} ${Math.round((now - last[0]!.created) / 3600)} h ago` : "nothing sent yet" });
    // the sending domain is still verified at Resend (a key with sending access only cannot see this)
    try {
      const d = await fetch("https://api.resend.com/domains", { headers: { authorization: `Bearer ${env.RESEND_API_KEY}` } });
      if (d.ok) {
        const from = /@([^>\s]+)/.exec(env.MAIL_FROM ?? "aside@filiposakkq.dev")?.[1] ?? "filiposakkq.dev";
        const dom = ((await d.json()) as { data?: { name: string; status: string }[] }).data?.find((x) => x.name === from);
        out.push({ key: "domain", label: "Sending domain", ok: dom ? dom.status === "verified" : false, detail: dom ? `${from}: ${dom.status}` : `${from} is not in Resend` });
      } else out.push({ key: "domain", label: "Sending domain", ok: null, detail: "the key may only send, so the domain cannot be checked" });
    } catch {
      out.push({ key: "domain", label: "Sending domain", ok: null, detail: "Resend did not answer" });
    }
  }
  // the daily job ran in the last 26 hours
  const ran = Number(await setting("last_daily")) || 0;
  out.push({ key: "daily", label: "Daily job", ok: ran ? now - ran < 26 * 3600 : null, detail: ran ? `last run ${Math.round((now - ran) / 3600)} h ago` : "has not run yet since this version" });
  out.push({ key: "turnstile", label: "Bot check (Turnstile)", ok: !!env.TURNSTILE_SECRET, detail: env.TURNSTILE_SECRET ? "set up" : "TURNSTILE_SECRET is not set" });
  out.push({ key: "salt", label: "HASH_SALT", ok: !!env.HASH_SALT, detail: env.HASH_SALT ? "set" : "not set: a fixed fallback is used" });
  out.push({ key: "push", label: "Phone notifications", ok: pushConfigured(env) ? true : null, detail: pushConfigured(env) ? (env.NTFY_TOPIC ? "ntfy" : "Telegram") : "not set up (NTFY_TOPIC or Telegram)" });
  return out;
}

/** Every hour: the health checks; a failing one sends a phone notification (at most once a day each), and one when it works again. */
export async function hourly(env: Env, now: number): Promise<{ alerts: number }> {
  let alerts = 0;
  const watched = ["database", "mail", "domain", "daily"];
  for (const c of await health(env, now)) {
    if (!watched.includes(c.key)) continue;
    const k = "alert_" + c.key;
    const since = Number((await env.DB.prepare("SELECT value FROM settings WHERE key = ?").bind(k).first<{ value: string }>())?.value ?? 0);
    if (c.ok === false && now - since >= DAY) {
      await push(env, "health", `Aside website: ${c.label} needs a look`, c.detail, { urgent: true, path: "/admin#overview" });
      await env.DB.prepare("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value").bind(k, String(now)).run();
      alerts++;
    } else if (c.ok !== false && since) {
      await push(env, "health", `Aside website: ${c.label} works again`, c.detail, { path: "/admin#overview" });
      await env.DB.prepare("DELETE FROM settings WHERE key = ?").bind(k).run();
    }
  }
  return { alerts };
}
