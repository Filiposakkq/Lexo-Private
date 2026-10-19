-- Limits per network address: a counter per (hashed address, call, hour). The address is hashed with the day and the rows are deleted after a few hours, so nothing here identifies a person for long.
CREATE TABLE rate_limits (
  bucket TEXT PRIMARY KEY,
  n      INTEGER NOT NULL,
  at     INTEGER NOT NULL
);
CREATE INDEX rate_limits_at ON rate_limits(at);
