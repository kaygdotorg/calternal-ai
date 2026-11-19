CREATE TABLE group_storage_limits(
    group_id TEXT PRIMARY KEY,
    text_bytes INTEGER CHECK(text_bytes>=0),
    binary_bytes INTEGER CHECK(binary_bytes>=0),
    total_bytes INTEGER CHECK(total_bytes>=0)
) STRICT, WITHOUT ROWID;
