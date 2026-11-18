CREATE TABLE storage_usage(
    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
    text_bytes INTEGER NOT NULL CHECK(text_bytes>=0),
    binary_bytes INTEGER NOT NULL CHECK(binary_bytes>=0),
    text_limit INTEGER CHECK(text_limit>=0),
    binary_limit INTEGER CHECK(binary_limit>=0)
) STRICT;

INSERT INTO storage_usage(singleton,text_bytes,binary_bytes)
SELECT 1,
       COALESCE(SUM(CASE WHEN json_extract(k.definition,'$[1].format')='markdown'
                         THEN length(v.bytes) ELSE 0 END),0),
       COALESCE(SUM(CASE WHEN json_extract(k.definition,'$[1].format')='markdown'
                         THEN 0 ELSE length(v.bytes) END),0)
FROM items i
JOIN versions v ON v.item=i.id AND v.seq=i.seq
LEFT JOIN kinds k ON k.kind=i.kind;

-- The writer advances items.seq immediately before inserting its body.
-- Previous revisions remain in history and are never charged a second time.
CREATE TRIGGER storage_usage_revision AFTER INSERT ON versions
WHEN NEW.seq=(SELECT seq FROM items WHERE id=NEW.item)
BEGIN
    UPDATE storage_usage
    SET (text_bytes,binary_bytes) = (
        SELECT text_bytes + CASE WHEN markdown THEN delta ELSE 0 END,
               binary_bytes + CASE WHEN markdown THEN 0 ELSE delta END
        FROM (
            SELECT COALESCE(json_extract(k.definition,'$[1].format')='markdown',0) AS markdown,
                   length(NEW.bytes) - COALESCE((
                       SELECT length(v.bytes) FROM versions v
                       WHERE v.item=NEW.item AND v.seq<NEW.seq
                       ORDER BY v.seq DESC LIMIT 1
                   ),0) AS delta
            FROM items i LEFT JOIN kinds k ON k.kind=i.kind
            WHERE i.id=NEW.item
        )
    )
    WHERE singleton=1;
END;

-- Trash still owns its current body. The purge transaction deletes all
-- revisions while items.seq still identifies the last charged head.
CREATE TRIGGER storage_usage_purge AFTER DELETE ON versions
WHEN OLD.seq=(SELECT seq FROM items WHERE id=OLD.item)
BEGIN
    UPDATE storage_usage
    SET (text_bytes,binary_bytes) = (
        SELECT text_bytes - CASE WHEN markdown THEN length(OLD.bytes) ELSE 0 END,
               binary_bytes - CASE WHEN markdown THEN 0 ELSE length(OLD.bytes) END
        FROM (
            SELECT COALESCE(json_extract(k.definition,'$[1].format')='markdown',0) AS markdown
            FROM items i LEFT JOIN kinds k ON k.kind=i.kind
            WHERE i.id=OLD.item
        )
    )
    WHERE singleton=1;
END;


-- Existing data stays readable and deletable after an operator lowers a limit.
-- Only positive deltas beyond the corresponding ceiling are refused.
CREATE TRIGGER storage_usage_text_limit BEFORE UPDATE OF text_bytes ON storage_usage
WHEN NEW.text_bytes>OLD.text_bytes AND NEW.text_bytes>OLD.text_limit
BEGIN SELECT RAISE(ABORT, 'text storage byte limit'); END;
CREATE TRIGGER storage_usage_binary_limit BEFORE UPDATE OF binary_bytes ON storage_usage
WHEN NEW.binary_bytes>OLD.binary_bytes AND NEW.binary_bytes>OLD.binary_limit
BEGIN SELECT RAISE(ABORT, 'binary storage byte limit'); END;
