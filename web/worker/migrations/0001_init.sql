-- The database of the website (aside-web). Nothing here is shared with the licence server.

-- Settings changed in the admin: the round of the beta programs and its places.
CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
INSERT INTO settings (key, value) VALUES ('round', 'Round 1'), ('open', '1'), ('closes', ''), ('places_test', '4'), ('places_licence', '3');

-- Beta applications. `code` is a secret given to the applicant once accepted: the link to report bugs (testers) or to answer the feedback (licences).
CREATE TABLE applications (
  id           TEXT PRIMARY KEY,
  route        TEXT NOT NULL,              -- test | licence
  created      INTEGER NOT NULL,
  nick         TEXT NOT NULL,
  email        TEXT NOT NULL,
  lang         TEXT NOT NULL DEFAULT '',
  answers      TEXT NOT NULL,              -- JSON: [[label, value], ...] as the applicant saw them
  state        TEXT NOT NULL DEFAULT 'new', -- new | accepted | waitlist | declined
  note         TEXT NOT NULL DEFAULT '',
  decided      INTEGER,
  code         TEXT UNIQUE,
  trial_start  INTEGER,                    -- licences: the day the 30-day trial began (starts the feedback schedule)
  fb2_sent     INTEGER, fb2_reminded INTEGER, fb2_done INTEGER,
  fb4_sent     INTEGER, fb4_reminded INTEGER, fb4_done INTEGER,
  ip_hash      TEXT NOT NULL DEFAULT ''
);
CREATE INDEX applications_state ON applications(route, state, created);

-- Answers of the feedback after 2 and 4 weeks.
CREATE TABLE feedback (
  id             INTEGER PRIMARY KEY AUTOINCREMENT,
  application_id TEXT NOT NULL REFERENCES applications(id),
  which          TEXT NOT NULL,            -- 2w | 4w
  created        INTEGER NOT NULL,
  answers        TEXT NOT NULL             -- JSON
);

-- The roadmap and its votes (one vote per item per browser; the browser keeps a random id, only its hash is stored).
CREATE TABLE roadmap (
  id      TEXT PRIMARY KEY,
  col     TEXT NOT NULL,                   -- now | next | later | ideas
  title   TEXT NOT NULL,
  text    TEXT NOT NULL DEFAULT '',
  tier    TEXT NOT NULL DEFAULT 'free',    -- free | pro | web
  sort    INTEGER NOT NULL DEFAULT 0,
  extra   INTEGER NOT NULL DEFAULT 0,      -- votes added by hand (e-mails, before the server)
  created INTEGER NOT NULL
);
CREATE TABLE votes (
  item_id TEXT NOT NULL REFERENCES roadmap(id) ON DELETE CASCADE,
  voter   TEXT NOT NULL,
  at      INTEGER NOT NULL,
  PRIMARY KEY (item_id, voter)
);

-- Bug reports of accepted testers. A screenshot is kept in the row (compressed in the browser, at most about 1 MB).
CREATE TABLE bugs (
  id             INTEGER PRIMARY KEY AUTOINCREMENT,
  application_id TEXT NOT NULL REFERENCES applications(id),
  created        INTEGER NOT NULL,
  title          TEXT NOT NULL,
  steps          TEXT NOT NULL,
  expected       TEXT NOT NULL DEFAULT '',
  actual         TEXT NOT NULL DEFAULT '',
  version        TEXT NOT NULL DEFAULT '',
  severity       TEXT NOT NULL DEFAULT 'normal', -- crash | broken | normal | cosmetic
  state          TEXT NOT NULL DEFAULT 'new',    -- new | confirmed | fixed | cant-reproduce | duplicate
  note           TEXT NOT NULL DEFAULT '',
  image          BLOB,
  image_type     TEXT NOT NULL DEFAULT ''
);

-- Translations sent in for the showcase; shown on the website once approved.
CREATE TABLE showcase (
  id         TEXT PRIMARY KEY,
  created    INTEGER NOT NULL,
  state      TEXT NOT NULL DEFAULT 'pending', -- pending | approved | rejected
  game       TEXT NOT NULL,
  lang       TEXT NOT NULL,
  link       TEXT NOT NULL,
  translator TEXT NOT NULL,
  email      TEXT NOT NULL DEFAULT '',
  blurb      TEXT NOT NULL DEFAULT '',
  lines      TEXT NOT NULL DEFAULT '',
  sort       INTEGER NOT NULL DEFAULT 0,
  ip_hash    TEXT NOT NULL DEFAULT ''
);
INSERT INTO showcase (id, created, state, game, lang, link, translator, blurb, lines, sort)
VALUES ('being-a-dik-cs', 0, 'approved', 'Being a DIK', 'Czech', 'https://badik.filiposakkq.dev', 'filiposakkq', 'The translation Aside was born from, made with the BaDIK Translation Manager.', '45 291', 0);

-- Messages from the contact form.
CREATE TABLE messages (
  id      INTEGER PRIMARY KEY AUTOINCREMENT,
  created INTEGER NOT NULL,
  name    TEXT NOT NULL,
  email   TEXT NOT NULL,
  topic   TEXT NOT NULL DEFAULT '',
  body    TEXT NOT NULL,
  state   TEXT NOT NULL DEFAULT 'new',     -- new | answered | archived
  ip_hash TEXT NOT NULL DEFAULT ''
);

-- Releases shown on the website; downloads go through /api/download/<id> (counted, then sent to `url`).
CREATE TABLE releases (
  id        TEXT PRIMARY KEY,
  version   TEXT NOT NULL,
  date      TEXT NOT NULL,
  channel   TEXT NOT NULL DEFAULT 'stable', -- stable | beta
  title     TEXT NOT NULL DEFAULT '',
  notes     TEXT NOT NULL DEFAULT '',       -- one change per line; a line may start with "new:", "improved:", "fixed:"
  url       TEXT NOT NULL DEFAULT '',
  sha256    TEXT NOT NULL DEFAULT '',
  size      TEXT NOT NULL DEFAULT '',
  published INTEGER NOT NULL DEFAULT 0,
  downloads INTEGER NOT NULL DEFAULT 0,
  created   INTEGER NOT NULL
);

-- Limits against floods: one row per request of a kind, removed after two days.
CREATE TABLE hits (
  ip_hash TEXT NOT NULL,
  kind    TEXT NOT NULL,
  at      INTEGER NOT NULL
);
CREATE INDEX hits_kind ON hits(kind, ip_hash, at);
