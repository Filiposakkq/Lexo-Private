import type { LicenceRow } from "./licence.ts";

export interface ActivationRow {
  licence_id: string;
  device: string;
  created: number;
  last_seen: number;
  released: number | null;
  name: string;
  app_version: string;
}

export interface EventRow {
  id: number;
  at: number;
  licence_id: string | null;
  kind: string;
  detail: string;
}

export const byKeyHash = (db: D1Database, hash: string) => db.prepare("SELECT * FROM licences WHERE key_hash = ?").bind(hash).first<LicenceRow>();
export const byId = (db: D1Database, id: string) => db.prepare("SELECT * FROM licences WHERE id = ?").bind(id).first<LicenceRow>();
export const byOrder = (db: D1Database, ref: string) => db.prepare("SELECT * FROM licences WHERE order_ref = ?").bind(ref).first<LicenceRow>();

export async function insertLicence(db: D1Database, l: LicenceRow): Promise<void> {
  await db
    .prepare("INSERT INTO licences (id, key_hash, email, order_ref, major, features, check_days, max_devices, state, created, note) VALUES (?,?,?,?,?,?,?,?,?,?,?)")
    .bind(l.id, l.key_hash, l.email, l.order_ref, l.major, l.features, l.check_days, l.max_devices, l.state, l.created, l.note)
    .run();
}

export const setState = (db: D1Database, id: string, state: string) => db.prepare("UPDATE licences SET state = ? WHERE id = ?").bind(state, id).run();
export const setKeyHash = (db: D1Database, id: string, hash: string) => db.prepare("UPDATE licences SET key_hash = ? WHERE id = ?").bind(hash, id).run();
export const setCheckDays = (db: D1Database, id: string, days: number) => db.prepare("UPDATE licences SET check_days = ? WHERE id = ?").bind(days, id).run();
export const setMaxDevices = (db: D1Database, id: string, n: number) => db.prepare("UPDATE licences SET max_devices = ? WHERE id = ?").bind(n, id).run();
export const setContact = (db: D1Database, id: string, email: string, note: string) =>
  db.prepare("UPDATE licences SET email = ?, note = ? WHERE id = ?").bind(email, note, id).run();

/** Removes what the events keep about a person: the e-mail address of the mailing of the key, and the notes of the owner. */
export async function scrubEvents(db: D1Database, id: string): Promise<void> {
  await db.prepare("UPDATE events SET detail = '' WHERE licence_id = ? AND kind IN ('key-mailed', 'key-not-mailed')").bind(id).run();
  await db.prepare("DELETE FROM events WHERE licence_id = ? AND kind = 'note'").bind(id).run();
}

/** The notes of the owner on a licence (a history, newest first). */
export const notes = async (db: D1Database, id: string) =>
  (await db.prepare("SELECT * FROM events WHERE licence_id = ? AND kind = 'note' ORDER BY id DESC LIMIT 200").bind(id).all<EventRow>()).results;

export const deleteNote = (db: D1Database, id: string, noteId: number) => db.prepare("DELETE FROM events WHERE id = ? AND licence_id = ? AND kind = 'note'").bind(noteId, id).run();

/** The other licences that have a computer with the same id (a copied licence file, or one person with two licences). */
export async function sharedWith(db: D1Database, id: string, device: string): Promise<string[]> {
  return (await db.prepare("SELECT DISTINCT licence_id FROM activations WHERE device = ? AND licence_id <> ?").bind(device, id).all<{ licence_id: string }>()).results.map((r) => r.licence_id);
}

/** The events of one kind (for example the refused attempts), newest first. */
export const eventsByKind = async (db: D1Database, kind: string, limit = 100) =>
  (await db.prepare("SELECT * FROM events WHERE kind = ? ORDER BY id DESC LIMIT ?").bind(kind, limit).all<EventRow>()).results;

export const activations = async (db: D1Database, id: string) =>
  (await db.prepare("SELECT * FROM activations WHERE licence_id = ? ORDER BY created").bind(id).all<ActivationRow>()).results;

export const activation = (db: D1Database, id: string, device: string) =>
  db.prepare("SELECT * FROM activations WHERE licence_id = ? AND device = ?").bind(id, device).first<ActivationRow>();

export async function activeCount(db: D1Database, id: string): Promise<number> {
  const r = await db.prepare("SELECT COUNT(*) AS n FROM activations WHERE licence_id = ? AND released IS NULL").bind(id).first<{ n: number }>();
  return r?.n ?? 0;
}

/** Takes a place for a device (or takes the place of a released one again). An empty name or version keeps what is stored. */
/** Takes a place for a device in ONE statement that also checks the limit, so parallel activations cannot together get past `max`. Returns whether a place was taken. */
export async function takePlace(db: D1Database, id: string, device: string, now: number, max: number, name = "", version = ""): Promise<boolean> {
  const r = await db
    .prepare(
      `INSERT INTO activations (licence_id, device, created, last_seen, released, name, app_version)
       SELECT ?1, ?2, ?3, ?3, NULL, ?4, ?5 WHERE (SELECT COUNT(*) FROM activations WHERE licence_id = ?1 AND released IS NULL) < ?6
       ON CONFLICT(licence_id, device) DO UPDATE SET released = NULL, last_seen = excluded.last_seen,
         name = CASE WHEN excluded.name <> '' THEN excluded.name ELSE name END,
         app_version = CASE WHEN excluded.app_version <> '' THEN excluded.app_version ELSE app_version END`,
    )
    .bind(id, device, now, name, version, max)
    .run();
  return r.meta.changes > 0;
}

export const touch = (db: D1Database, id: string, device: string, now: number, name = "", version = "") =>
  db
    .prepare(
      `UPDATE activations SET last_seen = ?, name = CASE WHEN ? <> '' THEN ? ELSE name END, app_version = CASE WHEN ? <> '' THEN ? ELSE app_version END
       WHERE licence_id = ? AND device = ?`,
    )
    .bind(now, name, name, version, version, id, device)
    .run();

/** The computers that hold the licence now. */
export const activeDevices = async (db: D1Database, id: string) =>
  (await db.prepare("SELECT * FROM activations WHERE licence_id = ? AND released IS NULL ORDER BY created").bind(id).all<ActivationRow>()).results;

/** How many times the user released a computer in the last `days` days (the owner's releases do not count), and when the oldest of them was. */
export async function userReleases(db: D1Database, id: string, since: number): Promise<{ count: number; oldest: number | null }> {
  const r = await db
    .prepare("SELECT COUNT(*) AS n, MIN(at) AS oldest FROM events WHERE licence_id = ? AND kind IN ('deactivate', 'release-user') AND at > ?")
    .bind(id, since)
    .first<{ n: number; oldest: number | null }>();
  return { count: r?.n ?? 0, oldest: r?.oldest ?? null };
}

export const release = (db: D1Database, id: string, device: string, now: number) =>
  db.prepare("UPDATE activations SET released = ? WHERE licence_id = ? AND device = ? AND released IS NULL").bind(now, id, device).run();

/** Frees every place of a licence (the owner, after a key leaked). Returns how many computers were freed. */
export async function releaseAll(db: D1Database, id: string, now: number): Promise<number> {
  const r = await db.prepare("UPDATE activations SET released = ? WHERE licence_id = ? AND released IS NULL").bind(now, id).run();
  return r.meta.changes;
}

export const logEvent = (db: D1Database, now: number, licenceId: string | null, kind: string, detail = "") =>
  db.prepare("INSERT INTO events (at, licence_id, kind, detail) VALUES (?,?,?,?)").bind(now, licenceId, kind, detail).run();

export const events = async (db: D1Database, id: string, limit = 50) =>
  (await db.prepare("SELECT * FROM events WHERE licence_id = ? ORDER BY id DESC LIMIT ?").bind(id, limit).all<EventRow>()).results;

export const recentEvents = async (db: D1Database, limit = 50) =>
  (await db.prepare("SELECT * FROM events ORDER BY id DESC LIMIT ?").bind(limit).all<EventRow>()).results;

export interface ListedLicence extends LicenceRow {
  devices: number;
  last_seen: number | null;
  /** How many times the user freed a computer (counts against the limit for the whole life of the licence). */
  user_releases: number;
  /** Whether the user already replaced the key of a lost one. */
  recovered: number;
  /** Why the licence was revoked (the detail of the last revoking), empty when it is not. */
  revoke_reason: string | null;
  /** Computers of this licence whose id is also on another licence. */
  shared_devices: number;
}

/** The licences for the admin list: with the number of devices and the last time any of them checked. `q` matches the id or the e-mail. */
export async function list(db: D1Database, q: string, limit = 200): Promise<ListedLicence[]> {
  const like = `%${q.replace(/[%_]/g, "")}%`;
  // the id, the e-mail, the note, the notes of the history, and the name or the id of a computer
  return (
    await db
      .prepare(
        `SELECT l.*, (SELECT COUNT(*) FROM activations a WHERE a.licence_id = l.id AND a.released IS NULL) AS devices,
                (SELECT MAX(a.last_seen) FROM activations a WHERE a.licence_id = l.id) AS last_seen,
                (SELECT COUNT(*) FROM events e WHERE e.licence_id = l.id AND e.kind IN ('deactivate', 'release-user')) AS user_releases,
                (SELECT COUNT(*) FROM events e WHERE e.licence_id = l.id AND e.kind = 'key-recovered') AS recovered,
                (SELECT detail FROM events e WHERE e.licence_id = l.id AND e.kind = 'revoked' ORDER BY e.id DESC LIMIT 1) AS revoke_reason,
                (SELECT COUNT(*) FROM activations a WHERE a.licence_id = l.id AND EXISTS (SELECT 1 FROM activations b WHERE b.device = a.device AND b.licence_id <> a.licence_id)) AS shared_devices
         FROM licences l
         WHERE l.id LIKE ?1 OR l.email LIKE ?1 OR l.note LIKE ?1
            OR EXISTS (SELECT 1 FROM activations a WHERE a.licence_id = l.id AND (a.device LIKE ?1 OR a.name LIKE ?1))
            OR EXISTS (SELECT 1 FROM events e WHERE e.licence_id = l.id AND e.kind = 'note' AND e.detail LIKE ?1)
         ORDER BY l.created DESC LIMIT ?2`,
      )
      .bind(like, limit)
      .all<ListedLicence>()
  ).results;
}

export async function stats(db: D1Database, now: number) {
  const one = async (sql: string, ...args: unknown[]) => (await db.prepare(sql).bind(...args).first<{ n: number }>())?.n ?? 0;
  return {
    active: await one("SELECT COUNT(*) AS n FROM licences WHERE state = 'active'"),
    revoked: await one("SELECT COUNT(*) AS n FROM licences WHERE state = 'revoked'"),
    recovered: await one("SELECT COUNT(*) AS n FROM events WHERE kind = 'key-recovered'"),
    refused_day: await one("SELECT COUNT(*) AS n FROM events WHERE kind = 'refused' AND at > ?", now - 86400),
    devices: await one("SELECT COUNT(*) AS n FROM activations WHERE released IS NULL"),
    overdue: await one(
      `SELECT COUNT(*) AS n FROM licences l WHERE l.state = 'active' AND (SELECT MAX(a.last_seen) FROM activations a WHERE a.licence_id = l.id AND a.released IS NULL) < ? - l.check_days * 86400`,
      now,
    ),
  };
}

// ---------------------------------------------------------------- what the admin page shows beside the list

export interface Attention {
  id: string;
  email: string;
  reasons: string[];
}

/** Licences that deserve a look, with the reasons (the last 30 days of the log, and the state of the computers). */
export async function attention(db: D1Database, now: number): Promise<Attention[]> {
  const since = now - 30 * 86400;
  const by = new Map<string, string[]>();
  const add = (id: string | null, why: string) => {
    if (id) by.set(id, [...(by.get(id) ?? []), why]);
  };
  const rows = async <T>(sql: string, ...args: unknown[]) => (await db.prepare(sql).bind(...args).all<T>()).results;
  for (const r of await rows<{ licence_id: string; n: number }>("SELECT licence_id, COUNT(*) AS n FROM events WHERE kind = 'refused' AND detail LIKE 'device limit%' AND at > ? AND licence_id IS NOT NULL GROUP BY licence_id HAVING n >= 3", since))
    add(r.licence_id, `asked for more computers than allowed ${r.n} times`);
  for (const r of await rows<{ licence_id: string; n: number }>("SELECT licence_id, COUNT(*) AS n FROM events WHERE kind = 'refused' AND detail = 'revoked key used' AND at > ? AND licence_id IS NOT NULL GROUP BY licence_id", since))
    add(r.licence_id, `the key of a revoked licence was used ${r.n} times`);
  for (const r of await rows<{ licence_id: string; n: number }>("SELECT licence_id, COUNT(*) AS n FROM events WHERE kind IN ('deactivate', 'release-user') GROUP BY licence_id HAVING n >= 2"))
    add(r.licence_id, `freed computers ${r.n} times (the limit)`);
  for (const r of await rows<{ licence_id: string }>("SELECT DISTINCT a.licence_id AS licence_id FROM activations a JOIN activations b ON a.device = b.device AND a.licence_id <> b.licence_id WHERE a.released IS NULL"))
    add(r.licence_id, "a computer id is also on another licence (a copied licence file?)");
  for (const r of await rows<{ licence_id: string }>("SELECT DISTINCT licence_id FROM events WHERE kind = 'key-recovered' AND at > ?", since))
    add(r.licence_id, "the user replaced a lost key");
  const out: Attention[] = [];
  for (const [id, reasons] of by) {
    const l = await byId(db, id);
    if (l && l.state === "active") out.push({ id, email: l.email, reasons });
  }
  return out.sort((a, b) => b.reasons.length - a.reasons.length).slice(0, 50);
}

/** Active licences that were never activated, older than `days`. */
export async function notActivated(db: D1Database, now: number, days = 7) {
  return (
    await db
      .prepare("SELECT id, email, created FROM licences l WHERE state = 'active' AND created < ? AND NOT EXISTS (SELECT 1 FROM activations a WHERE a.licence_id = l.id) ORDER BY created LIMIT 50")
      .bind(now - days * 86400)
      .all<{ id: string; email: string; created: number }>()
  ).results;
}

/** The versions of Aside on the computers that hold a place now. */
export async function versions(db: D1Database): Promise<{ version: string; n: number }[]> {
  return (await db.prepare("SELECT CASE WHEN app_version = '' THEN 'unknown' ELSE app_version END AS version, COUNT(*) AS n FROM activations WHERE released IS NULL GROUP BY 1 ORDER BY n DESC").all<{ version: string; n: number }>()).results;
}

/** How often a limit per address hit in the last day, by call, and how many counters there are now. */
export async function limitsOverview(db: D1Database, now: number) {
  const by = (await db.prepare("SELECT detail AS route, COUNT(*) AS n FROM events WHERE kind = 'rate-limited' AND at > ? GROUP BY detail ORDER BY n DESC").bind(now - 86400).all<{ route: string; n: number }>()).results;
  const counters = (await db.prepare("SELECT COUNT(*) AS n FROM rate_limits").first<{ n: number }>())?.n ?? 0;
  return { blocked_day: by.reduce((s, r) => s + r.n, 0), by_route: by, counters };
}

/** All counters of the limits per address are deleted: an address that was blocked can go on at once. */
export const resetLimits = (db: D1Database) => db.prepare("DELETE FROM rate_limits").run();

/** Facts about the server for the page (never a secret, only whether it is set). */
export async function counts(db: D1Database) {
  const one = async (sql: string) => (await db.prepare(sql).first<{ n: number }>())?.n ?? 0;
  const lastMail = await db.prepare("SELECT MAX(at) AS at FROM events WHERE kind = 'key-mailed'").first<{ at: number | null }>();
  const lastMailFailed = await db.prepare("SELECT MAX(at) AS at FROM events WHERE kind = 'key-not-mailed'").first<{ at: number | null }>();
  return {
    licences: await one("SELECT COUNT(*) AS n FROM licences"),
    activations: await one("SELECT COUNT(*) AS n FROM activations"),
    events: await one("SELECT COUNT(*) AS n FROM events"),
    last_mailed: lastMail?.at ?? null,
    last_mail_failed: lastMailFailed?.at ?? null,
  };
}
