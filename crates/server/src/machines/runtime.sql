CREATE TABLE resource_assignments(
    lease TEXT NOT NULL REFERENCES leases(id) ON DELETE CASCADE,
    fence INTEGER NOT NULL,
    machine TEXT NOT NULL,
    PRIMARY KEY(lease,fence)
) STRICT, WITHOUT ROWID;

CREATE TABLE guest_runtime(
    machine TEXT NOT NULL,
    generation TEXT NOT NULL,
    account TEXT NOT NULL,
    scope_kind TEXT NOT NULL CHECK(scope_kind IN ('account','group')),
    scope_id TEXT NOT NULL,
    lease TEXT NOT NULL,
    fence INTEGER NOT NULL,
    started_ms INTEGER NOT NULL,
    running_ns INTEGER NOT NULL CHECK(running_ns>=0),
    state INTEGER NOT NULL CHECK(state BETWEEN 0 AND 2),
    received_ms INTEGER NOT NULL,
    PRIMARY KEY(machine,generation)
) STRICT, WITHOUT ROWID;
CREATE INDEX guest_runtime_scope ON guest_runtime(scope_kind,scope_id,received_ms);
CREATE INDEX guest_runtime_account ON guest_runtime(account,received_ms);
CREATE INDEX guest_runtime_retire ON guest_runtime(state,received_ms);
