import type { Env } from "./env.ts";
import * as mail from "./mail.ts";

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
  if (sent || stale.length || images)
    await env.DB.prepare("INSERT INTO log (at, who, action, detail) VALUES (?, 'system', 'daily', ?)").bind(now, `${sent} feedback e-mails, ${stale.length} unconfirmed removed, ${images} screenshots removed`).run();
  return { sent };
}
