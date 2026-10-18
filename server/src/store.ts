import type { LicenceRow } from "./licence.ts";

export interface ActivationRow {
  licence_id: string;
  device: string;
  created: number;
  last_seen: number;
  released: number | null;
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

export const activations = async (db: D1Database, id: string) =>
  (await db.prepare("SELECT * FROM activations WHERE licence_id = ? ORDER BY created").bind(id).all<ActivationRow>()).results;

export const activation = (db: D1Database, id: string, device: string) =>
  db.prepare("SELECT * FROM activations WHERE licence_id = ? AND device = ?").bind(id, device).first<ActivationRow>();

export async function activeCount(db: D1Database, id: string): Promise<number> {
  const r = await db.prepare("SELECT COUNT(*) AS n FROM activations WHERE licence_id = ? AND released IS NULL").bind(id).first<{ n: number }>();
  return r?.n ?? 0;
}

/** Takes a place for a device (or takes the place of a released one again). */
export const takePlace = (db: D1Database, id: string, device: string, now: number) =>
  db
    .prepare(
      "INSERT INTO activations (licence_id, device, created, last_seen, released) VALUES (?,?,?,?,NULL) ON CONFLICT(licence_id, device) DO UPDATE SET released = NULL, last_seen = excluded.last_seen",
    )
    .bind(id, device, now, now)
    .run();

export const touch = (db: D1Database, id: string, device: string, now: number) =>
  db.prepare("UPDATE activations SET last_seen = ? WHERE licence_id = ? AND device = ?").bind(now, id, device).run();

export const release = (db: D1Database, id: string, device: string, now: number) =>
  db.prepare("UPDATE activations SET released = ? WHERE licence_id = ? AND device = ? AND released IS NULL").bind(now, id, device).run();

export const logEvent = (db: D1Database, now: number, licenceId: string | null, kind: string, detail = "") =>
  db.prepare("INSERT INTO events (at, licence_id, kind, detail) VALUES (?,?,?,?)").bind(now, licenceId, kind, detail).run();

export const events = async (db: D1Database, id: string, limit = 50) =>
  (await db.prepare("SELECT * FROM events WHERE licence_id = ? ORDER BY id DESC LIMIT ?").bind(id, limit).all<EventRow>()).results;

export const recentEvents = async (db: D1Database, limit = 50) =>
  (await db.prepare("SELECT * FROM events ORDER BY id DESC LIMIT ?").bind(limit).all<EventRow>()).results;

export interface ListedLicence extends LicenceRow {
  devices: number;
  last_seen: number | null;
}

/** The licences for the admin list: with the number of devices and the last time any of them checked. `q` matches the id or the e-mail. */
export async function list(db: D1Database, q: string, limit = 200): Promise<ListedLicence[]> {
  const like = `%${q.replace(/[%_]/g, "")}%`;
  return (
    await db
      .prepare(
        `SELECT l.*, (SELECT COUNT(*) FROM activations a WHERE a.licence_id = l.id AND a.released IS NULL) AS devices,
                (SELECT MAX(a.last_seen) FROM activations a WHERE a.licence_id = l.id) AS last_seen
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
    devices: await one("SELECT COUNT(*) AS n FROM activations WHERE released IS NULL"),
    overdue: await one(
      `SELECT COUNT(*) AS n FROM licences l WHERE l.state = 'active' AND (SELECT MAX(a.last_seen) FROM activations a WHERE a.licence_id = l.id AND a.released IS NULL) < ? - l.check_days * 86400`,
      now,
    ),
  };
}
