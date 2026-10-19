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

/** Removes the e-mail address that some events keep in their detail (the mailing of the key). */
export const scrubEvents = (db: D1Database, id: string) =>
  db.prepare("UPDATE events SET detail = '' WHERE licence_id = ? AND kind IN ('key-mailed', 'key-not-mailed')").bind(id).run();

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
}

/** The licences for the admin list: with the number of devices and the last time any of them checked. `q` matches the id or the e-mail. */
export async function list(db: D1Database, q: string, limit = 200): Promise<ListedLicence[]> {
  const like = `%${q.replace(/[%_]/g, "")}%`;
  return (
    await db
      .prepare(
        `SELECT l.*, (SELECT COUNT(*) FROM activations a WHERE a.licence_id = l.id AND a.released IS NULL) AS devices,
                (SELECT MAX(a.last_seen) FROM activations a WHERE a.licence_id = l.id) AS last_seen,
                (SELECT COUNT(*) FROM events e WHERE e.licence_id = l.id AND e.kind IN ('deactivate', 'release-user')) AS user_releases,
                (SELECT COUNT(*) FROM events e WHERE e.licence_id = l.id AND e.kind = 'key-recovered') AS recovered
         FROM licences l WHERE l.id LIKE ? OR l.email LIKE ? ORDER BY l.created DESC LIMIT ?`,
      )
      .bind(like, like, limit)
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
