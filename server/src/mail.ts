import type { Env } from "./env.ts";

/** Mails the licence key to the buyer (Resend). Returns whether it was sent; without `RESEND_API_KEY` nothing is sent (the owner re-issues the key from the admin page). */
export async function mailKey(env: Env, to: string, key: string, id: string): Promise<boolean> {
  if (!env.RESEND_API_KEY || !to) return false;
  const text = [
    "Thank you for buying Aside Pro.",
    "",
    `Your licence key: ${key}`,
    "",
    "Enter it in Aside: Settings, Licence. The key works on two of your computers; you can free one in the same place.",
    `Licence number (for questions): ${id}`,
  ].join("\n");
  try {
    const r = await fetch("https://api.resend.com/emails", {
      method: "POST",
      headers: { authorization: `Bearer ${env.RESEND_API_KEY}`, "content-type": "application/json" },
      body: JSON.stringify({ from: env.MAIL_FROM ?? "Aside <licence@aside.filiposakkq.dev>", to, subject: "Your Aside Pro licence key", text }),
    });
    return r.ok;
  } catch {
    return false;
  }
}
