-- Ratings of applications, testing tasks, test builds, the newsletter, and pages edited in the admin (status, FAQ, blog).

-- Your own rating of an application (0 = none, 1 to 5 stars) and short labels, comma separated.
ALTER TABLE applications ADD COLUMN rating INTEGER NOT NULL DEFAULT 0;
ALTER TABLE applications ADD COLUMN tags TEXT NOT NULL DEFAULT '';

-- Testing tasks: "try this" for testers, ticked off on their page with a short result.
CREATE TABLE tasks (
  id      TEXT PRIMARY KEY,
  created INTEGER NOT NULL,
  title   TEXT NOT NULL,
  body    TEXT NOT NULL DEFAULT '',
  build   TEXT NOT NULL DEFAULT '',       -- the build it is about (optional)
  open    INTEGER NOT NULL DEFAULT 1,     -- closed tasks are no longer shown to testers
  sort    INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE task_results (
  task_id        TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  application_id TEXT NOT NULL,
  state          TEXT NOT NULL,           -- done | problem | skipped
  note           TEXT NOT NULL DEFAULT '',
  at             INTEGER NOT NULL,
  PRIMARY KEY (task_id, application_id)
);

-- Test builds handed out through the personal pages: a link, or the file itself kept in the database in pieces of 1 MB (base64).
CREATE TABLE builds (
  id        TEXT PRIMARY KEY,
  created   INTEGER NOT NULL,
  version   TEXT NOT NULL,
  notes     TEXT NOT NULL DEFAULT '',
  audience  TEXT NOT NULL DEFAULT 'test', -- test | licence | all
  url       TEXT NOT NULL DEFAULT '',     -- an outside link instead of a file
  filename  TEXT NOT NULL DEFAULT '',
  size      INTEGER NOT NULL DEFAULT 0,
  sha256    TEXT NOT NULL DEFAULT '',
  parts     INTEGER NOT NULL DEFAULT 0,   -- pieces of the file uploaded so far
  ready     INTEGER NOT NULL DEFAULT 0,   -- 1 once complete (and checked) or a link
  visible   INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE build_parts (
  build_id TEXT NOT NULL REFERENCES builds(id) ON DELETE CASCADE,
  n        INTEGER NOT NULL,
  data     TEXT NOT NULL,
  PRIMARY KEY (build_id, n)
);
CREATE TABLE build_downloads (
  build_id       TEXT NOT NULL,
  application_id TEXT NOT NULL,
  at             INTEGER NOT NULL
);
CREATE INDEX build_downloads_build ON build_downloads(build_id);

-- The newsletter: people confirm their e-mail first; every e-mail carries a link to unsubscribe or change topics.
CREATE TABLE subscribers (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  created     INTEGER NOT NULL,
  email       TEXT NOT NULL UNIQUE,
  topics      TEXT NOT NULL DEFAULT 'releases,rounds', -- releases | rounds | diary
  confirmed   INTEGER,
  token       TEXT,                       -- the confirmation link (cleared once used)
  unsub       TEXT NOT NULL UNIQUE,       -- the permanent link to change topics or unsubscribe
  ip_hash     TEXT NOT NULL DEFAULT ''
);
CREATE TABLE newsletters (
  id      INTEGER PRIMARY KEY AUTOINCREMENT,
  created INTEGER NOT NULL,
  topic   TEXT NOT NULL,
  subject TEXT NOT NULL,
  body    TEXT NOT NULL,
  total   INTEGER NOT NULL DEFAULT 0,
  sent    INTEGER NOT NULL DEFAULT 0
);

-- Questions and answers on the home page. Answers allow `code`, **bold** and [links](https://…).
CREATE TABLE faq (
  id        TEXT PRIMARY KEY,
  q         TEXT NOT NULL,
  a         TEXT NOT NULL,
  sort      INTEGER NOT NULL DEFAULT 0,
  published INTEGER NOT NULL DEFAULT 1
);
INSERT INTO faq (id, q, a, sort) VALUES
  ('change', 'Does Aside change my game?', 'No. Game folders are only read. Aside edits the translation files in `game/tl/<language>`, always after a backup, and keeps its own data in a small `.aside` folder next to them.', 10),
  ('games', 'Which games work?', 'Ren''Py 7.4 and 8 games, with or without a `tl` folder and with `.rpa` archives. It was tried on 8 real games with 9 to 687 script files. The scene preview is best on games made of full-screen renders.', 20),
  ('czech', 'Is it only for Czech?', 'No. Any language works. The interface is in English, and language packs add the rules of a language (Czech is the first). Without a pack the neutral rules apply.', 30),
  ('sdk', 'Do I need the Ren''Py SDK?', 'No. Aside unpacks archives itself and uses the game''s own engine to generate translation files. The SDK is only needed if you want to run Ren''Py''s lint from Aside.', 40),
  ('free', 'What is the catch with Free?', 'There is none. The whole translating workflow is free. Pro sells time savers for big projects, and you can try them on 20 lines at a time.', 50),
  ('windows', 'Why will Windows warn me about the installer?', 'The first builds are not code-signed yet, so SmartScreen shows "Windows protected your PC". Choose **More info**, then **Run anyway**. Each file comes with a SHA-256 checksum.', 60);

-- Posts of the dev diary written in the admin (the posts in web/templates/blog.html stay; the page shows both by date).
CREATE TABLE posts (
  id        TEXT PRIMARY KEY,             -- the address: blog.html#<id>
  created   INTEGER NOT NULL,
  date      TEXT NOT NULL,                -- YYYY-MM-DD
  title     TEXT NOT NULL,
  dek       TEXT NOT NULL DEFAULT '',     -- one sentence under the title
  tag       TEXT NOT NULL DEFAULT '',
  body      TEXT NOT NULL,                -- paragraphs, "### " headings, "- " lists, ``` code, `code`, **bold**, [links](https://…)
  published INTEGER NOT NULL DEFAULT 0
);

-- status_*: the "now" strip of the website; push_events: what sends a phone notification; last_daily: when the daily job last ran.
INSERT INTO settings (key, value) VALUES
  ('status_on', '0'), ('status_build', ''), ('status_next', ''), ('status_note', ''),
  ('push_events', 'applications,crash,messages,health'), ('last_daily', '0');
