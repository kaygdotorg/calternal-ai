-- The test uses an isolated migration list while the shared content migration
-- is assigned by the item writer lane.
CREATE TABLE contribution_calls (
    call TEXT PRIMARY KEY,
    execution TEXT NOT NULL UNIQUE,
    generation INTEGER NOT NULL CHECK(generation > 0),
    allocated INTEGER NOT NULL CHECK(allocated >= 0),
    next INTEGER NOT NULL DEFAULT 1 CHECK(next > 0),
    claimed INTEGER NOT NULL DEFAULT 0 CHECK(claimed IN (0, 1)),
    terminal INTEGER NOT NULL DEFAULT 0 CHECK(terminal IN (0, 1)),
    expires INTEGER,
    rate_ms INTEGER NOT NULL DEFAULT 0,
    byte_credit INTEGER NOT NULL DEFAULT 0 CHECK(byte_credit >= 0),
    frame_credit INTEGER NOT NULL DEFAULT 0 CHECK(frame_credit >= 0)
) WITHOUT ROWID, STRICT;

CREATE INDEX contribution_calls_expiration ON contribution_calls(expires);

CREATE TABLE contribution_frames (
    call TEXT NOT NULL REFERENCES contribution_calls(call) ON DELETE CASCADE,
    seq INTEGER NOT NULL CHECK(seq > 0),
    payload BLOB NOT NULL,
    producer INTEGER NOT NULL CHECK(producer IN (0, 1)),
    PRIMARY KEY(call, seq)
) WITHOUT ROWID, STRICT;

CREATE TABLE contribution_budget (
    singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
    bytes INTEGER NOT NULL DEFAULT 0 CHECK(bytes >= 0),
    calls INTEGER NOT NULL DEFAULT 0 CHECK(calls >= 0),
    rate_ms INTEGER NOT NULL DEFAULT 0,
    byte_credit INTEGER NOT NULL DEFAULT 0 CHECK(byte_credit >= 0),
    frame_credit INTEGER NOT NULL DEFAULT 0 CHECK(frame_credit >= 0)
) STRICT;
INSERT INTO contribution_budget(singleton) VALUES(1);
