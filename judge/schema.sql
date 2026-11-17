CREATE TABLE IF NOT EXISTS submissions (
  id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  github_id INTEGER NOT NULL,
  login TEXT NOT NULL,
  n INTEGER NOT NULL,
  moves TEXT NOT NULL,
  length INTEGER NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('accepted', 'not_shorter', 'invalid')),
  reason TEXT,
  commit_sha TEXT
);
CREATE INDEX IF NOT EXISTS submissions_user_time ON submissions(github_id, created_at);
CREATE INDEX IF NOT EXISTS submissions_record ON submissions(n, status, length);
CREATE INDEX IF NOT EXISTS submissions_outbox ON submissions(commit_sha, created_at);

CREATE TABLE IF NOT EXISTS oauth_states (
  state TEXT PRIMARY KEY,
  expires_at INTEGER NOT NULL
);

-- A short lease serializes delivery to GitHub; expired leases recover after a crash.
CREATE TABLE IF NOT EXISTS delivery_lock (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  owner TEXT NOT NULL,
  expires_at INTEGER NOT NULL
);
