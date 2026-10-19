import type { Env } from "./env.ts";

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c] ?? c);

/** The e-mail with the licence key: a plain text and an HTML version (the same content; the HTML is table-based with inline styles because mail programs ignore the rest). */
export function keyEmail(key: string, id: string): { subject: string; text: string; html: string } {
  const text = [
    "Thank you for buying Aside Pro!",
    "",
    "Your licence key:",
    "",
    `    ${key}`,
    "",
    "How to use it:",
    "1. Open Aside and go to Settings, Licence.",
    "2. Paste the key and press Activate.",
    "",
    "The key works on two of your computers. If you change computers, free a place in the same screen (Remove from this computer).",
    "",
    `Licence number (for questions): ${id}`,
    "Reply to this e-mail if you need help.",
    "",
    "If you did not buy Aside Pro, you can ignore this message.",
  ].join("\n");
  const box = "background:#f6f6f8;border:1px solid #e3e3e8;border-radius:10px;padding:16px;text-align:center";
  const html = `<!doctype html><html lang="en"><body style="margin:0;padding:24px 12px;background:#f2f2f5;font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;color:#1c1c1e">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0"><tr><td align="center">
<table role="presentation" width="520" cellpadding="0" cellspacing="0" style="max-width:520px;width:100%;background:#ffffff;border-radius:14px;overflow:hidden;border:1px solid #e3e3e8">
<tr><td style="background:#ff315c;padding:20px 28px"><span style="display:inline-block;width:30px;height:30px;line-height:30px;text-align:center;background:#ffffff;color:#ff315c;border-radius:8px;font-weight:700;font-size:18px;vertical-align:middle">A</span><span style="color:#ffffff;font-size:20px;font-weight:600;margin-left:10px;vertical-align:middle">Aside</span></td></tr>
<tr><td style="padding:28px">
<h1 style="margin:0 0 8px;font-size:21px;line-height:1.3">Thank you for buying Aside Pro</h1>
<p style="margin:0 0 20px;color:#555;font-size:15px;line-height:1.5">Here is your licence key. Keep this e-mail; you can use the key again if you change computers.</p>
<div style="${box}"><div style="font-size:12px;color:#6b6b70;text-transform:uppercase;letter-spacing:.06em;margin-bottom:6px">Licence key</div><div style="font-family:Consolas,'Courier New',monospace;font-size:19px;font-weight:700;letter-spacing:.04em;word-break:break-all">${esc(key)}</div></div>
<h2 style="margin:26px 0 8px;font-size:16px">How to use it</h2>
<ol style="margin:0;padding-left:20px;color:#333;font-size:15px;line-height:1.7"><li>Open Aside and go to <b>Settings, Licence</b>.</li><li>Paste the key and press <b>Activate</b>.</li></ol>
<p style="margin:18px 0 0;color:#555;font-size:14px;line-height:1.5">The key works on <b>two</b> of your computers. When you change computers, free a place in the same screen with <i>Remove from this computer</i>.</p>
</td></tr>
<tr><td style="padding:16px 28px;background:#fafafb;border-top:1px solid #e3e3e8;color:#6b6b70;font-size:13px;line-height:1.5">Licence number (for questions): <span style="font-family:Consolas,'Courier New',monospace">${esc(id)}</span><br>Need help? Just reply to this e-mail.<br>If you did not buy Aside Pro, you can ignore this message.</td></tr>
</table></td></tr></table></body></html>`;
  return { subject: "Your Aside Pro licence key", text, html };
}

/** Sends one mail (Resend). Returns whether it was sent; without `RESEND_API_KEY` nothing is sent. */
export interface SendResult {
  sent: boolean;
  /** Why not: `not_configured` (no RESEND_API_KEY), `no_address`, the status and the answer of Resend, or `network`. */
  reason?: string;
}

async function sendDetailed(env: Env, to: string, subject: string, text: string, html: string): Promise<SendResult> {
  if (!env.RESEND_API_KEY) return { sent: false, reason: "not_configured" };
  if (!to) return { sent: false, reason: "no_address" };
  const body: Record<string, unknown> = { from: env.MAIL_FROM ?? "Aside <licence@aside.filiposakkq.dev>", to, subject, text, html };
  if (env.REPLY_TO) body.reply_to = env.REPLY_TO;
  try {
    const r = await fetch("https://api.resend.com/emails", {
      method: "POST",
      headers: { authorization: `Bearer ${env.RESEND_API_KEY}`, "content-type": "application/json" },
      body: JSON.stringify(body),
    });
    if (r.ok) return { sent: true };
    return { sent: false, reason: `${r.status} ${(await r.text().catch(() => "")).slice(0, 200)}`.trim() };
  } catch {
    return { sent: false, reason: "network" };
  }
}

async function send(env: Env, to: string, subject: string, text: string, html: string): Promise<boolean> {
  return (await sendDetailed(env, to, subject, text, html)).sent;
}

/** A test mail from the admin page: tells whether Resend took it and, if not, why. */
export function sendTest(env: Env, to: string): Promise<SendResult> {
  const text = "This is a test e-mail from the licence server of Aside.\n\nIf you can read it, the keys and the codes for a lost key will arrive too.";
  const html = `<!doctype html><html><body style="font-family:system-ui,sans-serif"><p><b>Aside</b>: this is a test e-mail from the licence server.</p><p>If you can read it, the keys and the codes for a lost key will arrive too.</p></body></html>`;
  return sendDetailed(env, to, "Aside: test e-mail", text, html);
}

/** Mails the licence key to the buyer (Resend). Returns whether it was sent; without `RESEND_API_KEY` nothing is sent (the owner re-issues the key from the admin page). */
export async function mailKey(env: Env, to: string, key: string, id: string): Promise<boolean> {
  const { subject, text, html } = keyEmail(key, id);
  return send(env, to, subject, text, html);
}

/** The mail of a lost key: the code (a short text, valid for a few minutes), or the note that the automatic recovery was already used (`code` is `null`). */
export function codeEmail(code: string | null): { subject: string; text: string; html: string } {
  const lines = code
    ? [
        "Someone (hopefully you) asked in Aside for a new licence key.",
        "",
        "Your code:",
        "",
        `    ${code}`,
        "",
        "Enter it in Aside within 15 minutes. Your old key stops working only after you enter the code; the computers where Aside is already activated keep working.",
        "",
        "If it was not you, ignore this message: nothing has changed.",
      ]
    : [
        "Someone (hopefully you) asked in Aside for a new licence key.",
        "",
        "The key of this licence was already replaced once automatically, so it cannot be done again that way.",
        "Reply to this e-mail and we will help you.",
        "",
        "If it was not you, ignore this message: nothing has changed.",
      ];
  const html = `<!doctype html><html lang="en"><body style="margin:0;padding:24px 12px;background:#f2f2f5;font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;color:#1c1c1e"><table role="presentation" width="100%" cellpadding="0" cellspacing="0"><tr><td align="center"><table role="presentation" width="520" cellpadding="0" cellspacing="0" style="max-width:520px;width:100%;background:#ffffff;border-radius:14px;overflow:hidden;border:1px solid #e3e3e8"><tr><td style="background:#ff315c;padding:20px 28px"><span style="color:#ffffff;font-size:20px;font-weight:600">Aside</span></td></tr><tr><td style="padding:28px;font-size:15px;line-height:1.6">${lines
    .map((l) => (l.startsWith("    ") ? `<div style="background:#f6f6f8;border:1px solid #e3e3e8;border-radius:10px;padding:14px;text-align:center;font-family:Consolas,'Courier New',monospace;font-size:22px;font-weight:700;letter-spacing:.08em">${esc(l.trim())}</div>` : l ? `<p style="margin:0 0 12px">${esc(l)}</p>` : ""))
    .join("")}</td></tr></table></td></tr></table></body></html>`;
  return { subject: code ? "Your Aside code for a new licence key" : "Aside: your licence key", text: lines.join("\n"), html };
}

export async function mailCode(env: Env, to: string, code: string | null): Promise<boolean> {
  const { subject, text, html } = codeEmail(code);
  return send(env, to, subject, text, html);
}
