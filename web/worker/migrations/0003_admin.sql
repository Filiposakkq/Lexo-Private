-- The bigger admin: confirmed applications, removing people, a history of every e-mail, answers to bug reports, an activity log.

-- An application counts only once its author clicked the link in the confirmation e-mail (so nobody can apply in a stranger's name).
-- Unconfirmed applications are removed after 48 hours. Applications from before this change count as confirmed.
ALTER TABLE applications ADD COLUMN verified INTEGER;
ALTER TABLE applications ADD COLUMN verify_token TEXT;
-- The last time the tester or licence page was opened with the personal link.
ALTER TABLE applications ADD COLUMN last_seen INTEGER;
-- state may now also be 'removed' (taken out of the program after being accepted); the reason is kept here.
ALTER TABLE applications ADD COLUMN removed_reason TEXT NOT NULL DEFAULT '';
UPDATE applications SET verified = created;
CREATE UNIQUE INDEX applications_verify ON applications(verify_token);
CREATE INDEX applications_email ON applications(email);

-- The answer to a bug report, shown to the tester on their page (the private note stays private).
ALTER TABLE bugs ADD COLUMN reply TEXT NOT NULL DEFAULT '';
ALTER TABLE bugs ADD COLUMN replied INTEGER;
-- When the report was closed (fixed, can't reproduce, duplicate): screenshots of closed reports can be removed after a while.
ALTER TABLE bugs ADD COLUMN closed INTEGER;
CREATE INDEX bugs_app ON bugs(application_id);

-- Every e-mail the server sent or tried to send: automatic ones and yours.
CREATE TABLE mails (
  id             INTEGER PRIMARY KEY AUTOINCREMENT,
  created        INTEGER NOT NULL,
  application_id TEXT,                     -- the person it belongs to, if any
  to_email       TEXT NOT NULL,
  kind           TEXT NOT NULL,            -- verify | decision | feedback | reminder | message | bug-reply | link | removed | reply | owner
  subject        TEXT NOT NULL,
  body           TEXT NOT NULL,            -- the plain text
  sent           INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX mails_app ON mails(application_id, created);
CREATE INDEX mails_created ON mails(created);

-- What was done in the admin and by the daily job.
CREATE TABLE log (
  id     INTEGER PRIMARY KEY AUTOINCREMENT,
  at     INTEGER NOT NULL,
  who    TEXT NOT NULL,                    -- the admin's e-mail, "token" or "system"
  action TEXT NOT NULL,
  target TEXT NOT NULL DEFAULT '',
  detail TEXT NOT NULL DEFAULT ''
);
CREATE INDEX log_at ON log(at);

-- Saved texts for e-mails written in the admin. {nick} and {link} are filled in for each person.
CREATE TABLE templates (
  id      TEXT PRIMARY KEY,
  name    TEXT NOT NULL,
  subject TEXT NOT NULL,
  body    TEXT NOT NULL,
  created INTEGER NOT NULL
);
INSERT INTO templates (id, name, subject, body, created) VALUES
  ('build', 'New test build', 'A new Aside test build is ready', 'Hi {nick},

a new test build is ready. What changed:

• …

Please try it on your project and report anything odd on your page:
{link}', 0),
  ('key', 'Your key is on its way', 'Your Aside Pro key', 'Hi {nick},

your Aside Pro key comes in a separate e-mail in a moment. If you don''t see it, look in your spam folder too.', 0);

-- image_days: screenshots of closed reports are removed after so many days (0 = never); pages_open: the personal pages work;
-- apply_day: applications a day from everyone together.
INSERT INTO settings (key, value) VALUES ('image_days', '30'), ('pages_open', '1'), ('apply_day', '30');
