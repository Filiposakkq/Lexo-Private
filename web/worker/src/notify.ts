import type { Env } from "./env.ts";
import { site } from "./util.ts";

/* Phone notifications, separate from e-mail (which may land in spam or be the thing that broke).
   - ntfy (recommended, no account): install the ntfy app, subscribe to a long random topic name, put that name into the secret NTFY_TOPIC.
     NTFY_SERVER (default https://ntfy.sh) and NTFY_TOKEN (a private server or reserved topic) are optional.
   - Telegram: a bot from @BotFather (TELEGRAM_BOT_TOKEN) and your chat id (TELEGRAM_CHAT_ID).
   Which events send one is chosen in the admin (setting push_events). */

/** What can send a notification; the keys are stored in the setting push_events. */
export const EVENTS: Record<string, string> = {
  applications: "New confirmed application",
  crash: "Bug report marked as crash",
  bugs: "Every bug report",
  messages: "Contact message",
  feedback: "Licence feedback",
  showcase: "Showcase entry",
  subscribers: "New newsletter subscriber",
  health: "Something is broken (e-mails, database, daily job)",
};

export const pushConfigured = (env: Env) => !!env.NTFY_TOPIC || !!(env.TELEGRAM_BOT_TOKEN && env.TELEGRAM_CHAT_ID);

export async function enabled(env: Env, event: string): Promise<boolean> {
  const r = await env.DB.prepare("SELECT value FROM settings WHERE key = 'push_events'").first<{ value: string }>();
  return (r?.value ?? "applications,crash,messages,health").split(",").includes(event);
}

/** Sends a phone notification for an event if it is switched on. `force` skips the switch (the test button). Never throws. */
export async function push(env: Env, event: string, title: string, text: string, opts: { urgent?: boolean; force?: boolean; path?: string } = {}): Promise<boolean> {
  if (!pushConfigured(env)) return false;
  try {
    if (!opts.force && !(await enabled(env, event))) return false;
  } catch {
    // the database itself may be what is broken; still tell
  }
  const click = site(env) + (opts.path ?? "/admin");
  let ok = false;
  if (env.NTFY_TOPIC) {
    try {
      const server = (env.NTFY_SERVER || "https://ntfy.sh").replace(/\/+$/, "");
      const headers: Record<string, string> = { Title: encodeHeader(title), Priority: opts.urgent ? "high" : "default", Tags: opts.urgent ? "warning" : "bell", Click: click };
      if (env.NTFY_TOKEN) headers.Authorization = `Bearer ${env.NTFY_TOKEN}`;
      const r = await fetch(`${server}/${encodeURIComponent(env.NTFY_TOPIC)}`, { method: "POST", headers, body: text.slice(0, 3000) });
      ok = r.ok || ok;
    } catch {}
  }
  if (env.TELEGRAM_BOT_TOKEN && env.TELEGRAM_CHAT_ID) {
    try {
      const r = await fetch(`https://api.telegram.org/bot${env.TELEGRAM_BOT_TOKEN}/sendMessage`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ chat_id: env.TELEGRAM_CHAT_ID, text: `${opts.urgent ? "⚠️ " : ""}${title}\n${text}`.slice(0, 3500) + `\n${click}`, disable_web_page_preview: true }),
      });
      ok = r.ok || ok;
    } catch {}
  }
  return ok;
}

/** HTTP headers are Latin-1; ntfy reads RFC 2047 encoded words for anything else (names with accents). */
const encodeHeader = (s: string) => (/^[\x20-\x7e]*$/.test(s) ? s : `=?UTF-8?B?${btoa(String.fromCharCode(...new TextEncoder().encode(s)))}?=`);
