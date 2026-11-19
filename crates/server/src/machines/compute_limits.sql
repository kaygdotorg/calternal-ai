CREATE TABLE compute_limits(
    scope_kind TEXT NOT NULL CHECK(scope_kind IN ('account','group')),
    scope_id TEXT NOT NULL,
    hosted INTEGER NOT NULL CHECK(hosted IN (0,1)),
    memory_mib INTEGER CHECK(memory_mib BETWEEN 512 AND 65536 AND memory_mib%256=0),
    active_threads INTEGER CHECK(active_threads BETWEEN 1 AND 65536),
    queue_bound INTEGER CHECK(queue_bound BETWEEN 0 AND 65536),
    idle_seconds INTEGER CHECK(idle_seconds BETWEEN 1 AND 86400),
    runtime_seconds INTEGER CHECK(runtime_seconds BETWEEN 0 AND 2678400),
    reset_seconds INTEGER CHECK(reset_seconds BETWEEN 1 AND 2678400),
    CHECK((runtime_seconds IS NULL)=(reset_seconds IS NULL)),
    PRIMARY KEY(scope_kind,scope_id)
) STRICT, WITHOUT ROWID;
CREATE TABLE lease_resource_owners(
    lease TEXT PRIMARY KEY REFERENCES leases(id) ON DELETE CASCADE,
    scope_kind TEXT NOT NULL CHECK(scope_kind IN ('account','group')),
    scope_id TEXT NOT NULL
) STRICT, WITHOUT ROWID;
CREATE TABLE space_resource_groups(
    space TEXT PRIMARY KEY,
    group_id TEXT NOT NULL
) STRICT, WITHOUT ROWID;
