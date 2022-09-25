CREATE TABLE IF NOT EXISTS events (
  id INTEGER NOT NULL PRIMARY KEY,
  chat_id INTEGER NOT NULL,
  date TEXT NOT NULL,
  countdown_days INTEGER
);

CREATE TABLE IF NOT EXISTS subscription_types (
  id TEXT NOT NULL PRIMARY KEY
);

INSERT INTO subscription_types (id) VALUES ('comics'), ('events')
ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS subscriptions (
  subscription_type TEXT NOT NULL REFERENCES subscription_types(id),
  chat_id INTEGER NOT NULL,
  time TEXT NOT NULL,
  last_updated TEXT,

  PRIMARY KEY (chat_id, subscription_type)
);

CREATE TABLE IF NOT EXISTS autoreplies (
  id INTEGER NOT NULL PRIMARY KEY,
  chat_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  pattern_regex TEXT NOT NULL,
  response_json TEXT NOT NULL,

  UNIQUE (chat_id, name)
);

CREATE TABLE IF NOT EXISTS chat_settings (
  chat_id INTEGER NOT NULL PRIMARY KEY,
  autoreply_chance REAL NOT NULL DEFAULT 0.5,
  sticker_lru_size INTEGER NOT NULL DEFAULT 20
);

CREATE TABLE IF NOT EXISTS seen_stickers (
  chat_id INTEGER NOT NULL,
  emoji TEXT NOT NULL,
  stickers_json TEXT NOT NULL,

  UNIQUE (chat_id, emoji)
);

CREATE TABLE IF NOT EXISTS google_logins (
  user_id INTEGER NOT NULL PRIMARY KEY,
  refresh_token TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS connected_calendars (
  -- Only one calendar can be connected per chat
  chat_id INTEGER NOT NULL PRIMARY KEY,
  user_id INTEGER NOT NULL,
  calendar_id TEXT NOT NULL
);
