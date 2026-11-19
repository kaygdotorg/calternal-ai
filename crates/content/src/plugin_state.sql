CREATE TABLE plugin_state(
    package TEXT NOT NULL,
    key TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision>0),
    schema INTEGER NOT NULL CHECK(schema>0),
    value TEXT NOT NULL CHECK(json_valid(value)),
    PRIMARY KEY(package,key)
) STRICT, WITHOUT ROWID;
CREATE TABLE plugin_state_receipts(
    package TEXT NOT NULL,
    request TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    outer_fingerprint TEXT,
    revision INTEGER NOT NULL CHECK(revision>0),
    committed_ms INTEGER NOT NULL,
    PRIMARY KEY(package,request)
) STRICT, WITHOUT ROWID;
CREATE INDEX plugin_state_receipts_retire ON plugin_state_receipts(package,committed_ms);
