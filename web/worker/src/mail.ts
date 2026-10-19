import type { Env } from "./env.ts";
import { site } from "./util.ts";

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c] ?? c);

export interface Mail {
  subject: string;
  /** Paragraphs; a paragraph that starts with "• " becomes a list item, one that is a URL a button. */
  lines: string[];
}

/** Plain text and HTML of a mail from the same paragraphs (the HTML is table-based with inline styles, as mail programs need). */
export function render(m: Mail): { text: string; html: string } {
  const text = m.lines.join("\n\n") + "\n\n— Filip from Aside · aside@filiposakkq.dev";
  const parts = m.lines.map((p) => {
    if (/^https?:\/\/\S+$/.test(p))
      return `<p style="margin:22px 0"><a href="${esc(p)}" style="display:inline-block;background:#ff315c;color:#ffffff;text-decoration:none;font-weight:600;padding:12px 20px;border-radius:999px">Open</a><br><span style="font-size:12px;color:#8a8a90;word-break:break-all">${esc(p)}</span></p>`;
    if (p.startsWith("• "))
      return p.split("\n").map((li) => `<p style="margin:0 0 6px;padding-left:14px;text-indent:-14px;color:#333;font-size:15px;line-height:1.55">${esc(li)}</p>`).join("") + '<div style="height:8px"></div>';
    return `<p style="margin:0 0 14px;color:#333;font-size:15px;line-height:1.6">${esc(p).replace(/\n/g, "<br>")}</p>`;
  });
  const html = `<!doctype html><html lang="en"><body style="margin:0;padding:24px 12px;background:#f4f1f2;font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;color:#1c1c1e">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0"><tr><td align="center">
<table role="presentation" width="540" cellpadding="0" cellspacing="0" style="max-width:540px;width:100%;background:#ffffff;border-radius:16px;overflow:hidden;border:1px solid #eadfe2">
<tr><td style="background:#33101a;padding:18px 26px"><span style="display:inline-block;width:30px;height:30px;line-height:30px;text-align:center;background:#ff315c;color:#ffffff;border-radius:8px;font-weight:700;font-size:17px;vertical-align:middle">A</span><span style="color:#ffffff;font-size:19px;font-weight:700;margin-left:10px;vertical-align:middle">aside</span></td></tr>
<tr><td style="padding:26px">
<h1 style="margin:0 0 16px;font-size:20px;line-height:1.3;color:#1c1c1e">${esc(m.subject)}</h1>
${parts.join("\n")}
</td></tr>
<tr><td style="padding:14px 26px;background:#faf7f8;border-top:1px solid #eadfe2;color:#8a8a90;font-size:12px;line-height:1.5">Aside, a translation manager for Ren'Py · Reply to this e-mail to write to the author.</td></tr>
</table></td></tr></table></body></html>`;
  return { text, html };
}

/** Who a mail belongs to and what kind it is, for the history of e-mails in the admin. Without it the mail is not recorded. */
export interface Meta { kind: string; app?: string | null }

/** Sends a mail through Resend and records it (with `meta`) in the history. Returns whether it was sent; without RESEND_API_KEY nothing is sent (the data is still saved). */
export async function send(env: Env, to: string, m: Mail, meta?: Meta): Promise<boolean> {
  if (!to) return false;
  const { text, html } = render(m);
  let ok = false;
  if (env.RESEND_API_KEY) {
    try {
      const r = await fetch("https://api.resend.com/emails", {
        method: "POST",
        headers: { authorization: `Bearer ${env.RESEND_API_KEY}`, "content-type": "application/json" },
        body: JSON.stringify({ from: env.MAIL_FROM ?? "Filip from Aside <aside@filiposakkq.dev>", to, subject: m.subject, text, html, reply_to: env.OWNER_EMAIL ?? "aside@filiposakkq.dev" }),
      });
      ok = r.ok;
    } catch {
      ok = false;
    }
  }
  if (meta)
    await env.DB.prepare("INSERT INTO mails (created, application_id, to_email, kind, subject, body, sent) VALUES (?,?,?,?,?,?,?)")
      .bind(Math.floor(Date.now() / 1000), meta.app ?? null, to, meta.kind, m.subject, text.slice(0, 20000), ok ? 1 : 0)
      .run();
  return ok;
}

/** A short notice to the owner (new application, message, bug, showcase entry). */
export const notifyOwner = (env: Env, subject: string, lines: string[]) =>
  send(env, env.OWNER_EMAIL ?? "aside@filiposakkq.dev", { subject, lines: [...lines, site(env) + "/admin"] });

/** The personal page of an accepted person: bug reports for testers, feedback for licences. */
export const pageLink = (env: Env, route: string, code: string | null) => (code ? `${site(env)}/${route === "test" ? "report" : "feedback"}.html#${code}` : "");

// ---------------------------------------------------------------- the mails to people

const ROUTE = { test: "testing", licence: "a sponsored licence" } as const;

export function verify(route: "test" | "licence", nick: string, round: string, token: string, env: Env): Mail {
  return {
    subject: "Confirm your Aside application",
    lines: [
      `Hi ${nick},`,
      `someone (hopefully you) applied for ${ROUTE[route]} in ${round} with this e-mail address. Please confirm it was you; only confirmed applications are read:`,
      `${site(env)}/beta.html#confirm=${token}`,
      "The link works for 48 hours. If you did not apply, ignore this e-mail and the application is deleted by itself.",
    ],
  };
}

export function decision(route: "test" | "licence", state: "accepted" | "waitlist" | "declined", nick: string, code: string, env: Env, extra: string): Mail {
  const tail = extra ? [extra] : [];
  if (state === "accepted" && route === "test")
    return {
      subject: "Welcome to the Aside test team",
      lines: [
        `Hi ${nick},`,
        "you are in! Thank you for helping Aside get ready for its first release.",
        ...tail,
        "The first test build comes in a separate e-mail. When something breaks, report it on your personal page (keep the link to yourself):",
        `${site(env)}/report.html#${code}`,
        "• Write the steps that lead to the problem.\n• Add a screenshot when you can.\n• Keep a backup of your translation; test builds can have bugs.",
        "Please don't share the builds or their links.",
      ],
    };
  if (state === "accepted")
    return {
      subject: "Your sponsored Aside Pro licence",
      lines: [
        `Hi ${nick},`,
        "your project was picked for a sponsored licence. Here is how it goes:",
        "• Now: 30 days of Aside Pro for your project. The key comes in a separate e-mail.\n• After 2 and 4 weeks: a short feedback form arrives by e-mail.\n• After your feedback: the full licence for this major version.",
        ...tail,
        "Your feedback page, if you want to write earlier:",
        `${site(env)}/feedback.html#${code}`,
      ],
    };
  if (state === "waitlist")
    return {
      subject: "You are on the Aside waiting list",
      lines: [`Hi ${nick},`, "thank you for your application. The places of this round are taken, so you are on the waiting list. When a place opens or the next round starts, you will hear from me first.", ...tail],
    };
  return {
    subject: "About your Aside application",
    lines: [`Hi ${nick},`, "thank you for applying. There are only a few places in this round and I could not offer you one this time. It is not a judgement of your work, and you are welcome to apply again in the next round.", ...tail],
  };
}

export function feedbackInvite(which: "2w" | "4w", nick: string, code: string, env: Env, reminder: boolean): Mail {
  const weeks = which === "2w" ? "two" : "four";
  return {
    subject: reminder ? "A reminder: your Aside feedback" : `${weeks[0]!.toUpperCase() + weeks.slice(1)} weeks with Aside: how is it going?`,
    lines: [
      `Hi ${nick},`,
      reminder
        ? `a few days ago I asked how your ${weeks} weeks with Aside went. It takes about three minutes and it decides what gets fixed first.`
        : `you have used Aside Pro for ${weeks} weeks now. Tell me honestly how it goes: what helps, what is in the way, what broke. It takes about three minutes.`,
      `${site(env)}/feedback.html#${code}`,
      which === "4w" ? "After this one, your full licence follows." : "Thank you!",
    ],
  };
}

export function removed(route: "test" | "licence", nick: string, extra: string): Mail {
  return {
    subject: route === "test" ? "About your place in the Aside test team" : "About your sponsored Aside licence",
    lines: [
      `Hi ${nick},`,
      route === "test"
        ? "I have taken you out of the Aside test team, and your bug report page no longer works. Thank you for the time you gave to Aside."
        : "I have ended your sponsored Aside licence, and your feedback page no longer works.",
      ...(extra ? [extra] : []),
      "If you think this is a mistake, just reply to this e-mail.",
    ],
  };
}

export function newLink(route: "test" | "licence", nick: string, link: string): Mail {
  return {
    subject: route === "test" ? "Your new bug report link" : "Your new feedback link",
    lines: [`Hi ${nick},`, "here is your new personal link. The old one no longer works; keep this one to yourself:", link],
  };
}

export function bugReply(nick: string, id: number, title: string, state: string, answer: string, link: string): Mail {
  return {
    subject: `Bug #${id}: ${title}`.slice(0, 140),
    lines: [`Hi ${nick},`, `about your report #${id} "${title}"` + (state ? ` (now: ${state})` : "") + ":", answer, "You can see all your reports on your page:", link].filter(Boolean),
  };
}

/** A mail written in the admin: {nick} and {link} are filled in; paragraphs are separated by an empty line. */
export function custom(subject: string, body: string, nick: string, link: string): Mail {
  const fill = (t: string) => t.replace(/\{nick\}/g, nick).replace(/\{link\}/g, link);
  return { subject: fill(subject), lines: fill(body).split(/\n\s*\n/).map((p) => p.trim()).filter(Boolean) };
}

export function reply(nick: string, body: string, original: string): Mail {
  return { subject: "Re: your message to Aside", lines: [`Hi ${nick},`, body, "— Your message —\n" + original.slice(0, 1500)] };
}
