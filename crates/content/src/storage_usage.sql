CREATE TABLE storage_usage(
    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
    text_bytes INTEGER NOT NULL CHECK(text_bytes>=0),
    binary_bytes INTEGER NOT NULL CHECK(binary_bytes>=0),
    text_limit INTEGER CHECK(text_limit>=0),
    binary_limit INTEGER CHECK(binary_limit>=0),
    total_limit INTEGER CHECK(total_limit>=0)
) STRICT;
