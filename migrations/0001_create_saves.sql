CREATE TABLE saves (
  player_id TEXT PRIMARY KEY,
  state TEXT NOT NULL,
  schema_version INTEGER NOT NULL,
  version INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  cafe_credit REAL NOT NULL
) STRICT;
