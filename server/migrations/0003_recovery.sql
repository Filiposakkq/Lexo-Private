-- A lost licence key (self-service): every request is logged by the hash of the e-mail address (for the limits: 1 per hour, 3 per day), and a pending code is kept as a hash.
CREATE TABLE recovery_requests (
  email_hash TEXT NOT NULL,
  at         INTEGER NOT NULL
);
CREATE INDEX recovery_requests_email ON recovery_requests(email_hash, at);

CREATE TABLE recovery_codes (
  licence_id TEXT PRIMARY KEY REFERENCES licences(id),
  code_hash  TEXT NOT NULL,
  expires    INTEGER NOT NULL,
  attempts   INTEGER NOT NULL DEFAULT 0
);
