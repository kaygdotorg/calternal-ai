CREATE TABLE resource_hooks(
    space TEXT PRIMARY KEY,
    package TEXT NOT NULL CHECK(length(package) BETWEEN 1 AND 128),
    checksum TEXT NOT NULL CHECK(length(checksum)=64),
    state_space TEXT
) STRICT, WITHOUT ROWID;
