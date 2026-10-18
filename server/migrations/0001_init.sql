-- One row per licence. Only a hash of the key is stored (the key itself is shown once, to the buyer).
CREATE TABLE licences (
  id           TEXT PRIMARY KEY,
  key_hash     TEXT NOT NULL UNIQUE,
  email        TEXT NOT NULL DEFAULT '',
  order_ref    TEXT,
  major        INTEGER NOT NULL,
  features     TEXT NOT NULL DEFAULT 'all',
  check_days   INTEGER NOT NULL DEFAULT 14,
  max_devices  INTEGER NOT NULL DEFAULT 2,
  state        TEXT NOT NULL DEFAULT 'active',
  created      INTEGER NOT NULL,
  note         TEXT NOT NULL DEFAULT ''
);
CREATE UNIQUE INDEX licences_order ON licences(order_ref) WHERE order_ref IS NOT NULL;

-- A device that holds a licence. `released` is set when the user (or the owner) frees the place.
CREATE TABLE activations (
  licence_id TEXT NOT NULL REFERENCES licences(id),
  device     TEXT NOT NULL,
  created    INTEGER NOT NULL,
  last_seen  INTEGER NOT NULL,
  released   INTEGER,
  PRIMARY KEY (licence_id, device)
);

-- The log: activations, checks, refusals, revocations, purchases (to see what happened and to trace a leak).
CREATE TABLE events (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  at         INTEGER NOT NULL,
  licence_id TEXT,
  kind       TEXT NOT NULL,
  detail     TEXT NOT NULL DEFAULT ''
);
CREATE INDEX events_licence ON events(licence_id, at);
