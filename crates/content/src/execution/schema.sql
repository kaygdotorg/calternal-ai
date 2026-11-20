-- Registered in the per-space content migration by the item-writer lane.
-- Settled intents are removed. A fenced head keeps its slot until teardown.
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
