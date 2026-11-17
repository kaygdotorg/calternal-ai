
CREATE TABLE users (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    username    TEXT NOT NULL UNIQUE COLLATE NOCASE,
    created_at  INTEGER NOT NULL
);
CREATE TABLE passkeys (
    cred_id      BLOB PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id),
    passkey      TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    last_used_at INTEGER
);
CREATE INDEX passkeys_user ON passkeys(user_id);
CREATE TABLE setup_tokens (
    hash        BLOB PRIMARY KEY,
    purpose     TEXT NOT NULL CHECK (purpose IN ('first-owner', 'recover')),
    user_id     TEXT REFERENCES users(id),
    created_at  INTEGER NOT NULL,
    expires_at  INTEGER NOT NULL,
    used_at     INTEGER
);
CREATE TABLE installations (
    id           TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id),
    kind         TEXT NOT NULL CHECK (kind IN ('browser', 'native', 'cli', 'agent', 'dav')),
    name         TEXT NOT NULL,
    token_hash   BLOB NOT NULL UNIQUE,
    created_at   INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    expires_at   INTEGER,
    revoked_at   INTEGER
);
CREATE INDEX installations_user ON installations(user_id);


ALTER TABLE passkeys ADD COLUMN name TEXT NOT NULL DEFAULT 'Passkey';

PRAGMA user_version=2;
