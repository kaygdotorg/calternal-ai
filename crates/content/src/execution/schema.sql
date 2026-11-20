-- Part of the per-space content.db baseline (no migration chain before the
-- Threads cutover; owner decision 2026-11-20). Settled intents are removed and
-- cascade to their attempt, launch binding and control ledger. A fenced head
-- keeps its slot until verified teardown.
CREATE TABLE execution_intents (
    intent TEXT PRIMARY KEY,
    job TEXT NOT NULL UNIQUE,
    item TEXT NOT NULL REFERENCES items(id),
    slot INTEGER NOT NULL CHECK(slot IN (0, 1)),
    actor TEXT NOT NULL,
    installation TEXT NOT NULL,
    pin TEXT NOT NULL CHECK(json_valid(pin)),
    claimed INTEGER NOT NULL DEFAULT 0 CHECK(claimed IN (0, 1) AND (slot = 0 OR claimed = 0)),
    UNIQUE(item, slot)
) WITHOUT ROWID, STRICT;

CREATE INDEX execution_intents_installation ON execution_intents(actor, installation);

-- The exact attempt row doubles as the cleanup record after fencing. A head
-- with cleanup pending keeps both the attempt and its admission reservation.
CREATE TABLE execution_attempts (
    execution TEXT PRIMARY KEY,
    intent TEXT NOT NULL UNIQUE REFERENCES execution_intents(intent) ON DELETE CASCADE,
    generation INTEGER NOT NULL CHECK(generation > 0),
    transition TEXT NOT NULL,
    end_reason TEXT CHECK(end_reason IN ('stopped', 'interrupted', 'completed', 'failed'))
) WITHOUT ROWID, STRICT;

-- Sends accepted while the updater fence is held or startup recovery runs.
-- Identities are allocated at hold time so the receipt never changes. Rows
-- hold no capacity and leave exactly once: admitted, refused or stopped.
-- Bounded by the per-space admission capacity (checked at hold).
CREATE TABLE execution_held (
    seq INTEGER PRIMARY KEY,
    intent TEXT NOT NULL UNIQUE,
    job TEXT NOT NULL UNIQUE,
    item TEXT NOT NULL REFERENCES items(id),
    actor TEXT NOT NULL,
    installation TEXT NOT NULL,
    pin TEXT NOT NULL CHECK(json_valid(pin))
) STRICT;

CREATE INDEX execution_held_item ON execution_held(item);

-- Committed launch reservation, written before StartEntry. Reconnect finds
-- the attempt only through this exact reservation on the same machine.
CREATE TABLE execution_launches (
    execution TEXT PRIMARY KEY REFERENCES execution_attempts(execution) ON DELETE CASCADE,
    generation INTEGER NOT NULL CHECK(generation > 0),
    reservation TEXT NOT NULL UNIQUE,
    machine TEXT NOT NULL,
    boot TEXT NOT NULL,
    ticket TEXT NOT NULL,
    call TEXT NOT NULL
) WITHOUT ROWID, STRICT;

-- Durable steer and Stop delivery ledger for one exact attempt. At most 32
-- unsettled steers plus one reserved Stop per attempt; rows go with the
-- attempt at settlement, receipts follow the ordinary receipt retention.
CREATE TABLE execution_controls (
    execution TEXT NOT NULL REFERENCES execution_attempts(execution) ON DELETE CASCADE,
    seq INTEGER NOT NULL CHECK(seq > 0),
    request_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('steer', 'stop')),
    text TEXT CHECK((kind = 'steer') = (text IS NOT NULL)),
    state TEXT NOT NULL DEFAULT 'pending'
        CHECK(state IN ('pending', 'sent', 'applied', 'refused', 'stale', 'undelivered')),
    PRIMARY KEY(execution, seq),
    UNIQUE(execution, request_id)
) WITHOUT ROWID, STRICT;

CREATE UNIQUE INDEX execution_controls_stop ON execution_controls(execution) WHERE kind = 'stop';
