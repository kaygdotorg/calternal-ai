-- The writer advances items.seq immediately before inserting its body.
-- Charge only committed heads and enforce ceilings only on new growth.
-- A lower ceiling must not prevent a shrink, purge, or unpublished rollback.
CREATE TRIGGER storage_usage_revision AFTER INSERT ON versions
WHEN NEW.seq=(SELECT seq FROM items WHERE id=NEW.item)
BEGIN
    UPDATE storage_usage
    SET (text_bytes,binary_bytes) = (
        SELECT CASE
                   WHEN delta>0 AND text_bytes+binary_bytes+delta>total_limit
                       THEN RAISE(ABORT, 'total storage byte limit')
                   WHEN delta>0 AND markdown AND text_bytes+delta>text_limit
                       THEN RAISE(ABORT, 'text storage byte limit')
                   WHEN delta>0 AND NOT markdown AND binary_bytes+delta>binary_limit
                       THEN RAISE(ABORT, 'binary storage byte limit')
                   ELSE text_bytes + CASE WHEN markdown THEN delta ELSE 0 END
               END,
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

-- A failed rename can rewind the head before deleting the unpublished revision.
-- Restore the previous contribution in the same transaction, even at a ceiling.
CREATE TRIGGER storage_usage_rewind AFTER UPDATE OF seq ON items
WHEN NEW.seq<OLD.seq
BEGIN
    UPDATE storage_usage
    SET (text_bytes,binary_bytes) = (
        SELECT text_bytes + CASE WHEN markdown THEN delta ELSE 0 END,
               binary_bytes + CASE WHEN markdown THEN 0 ELSE delta END
        FROM (
            SELECT COALESCE(json_extract(k.definition,'$[1].format')='markdown',0) AS markdown,
                   length(restored.bytes) - length(discarded.bytes) AS delta
            FROM versions restored JOIN versions discarded ON discarded.item=restored.item
            LEFT JOIN kinds k ON k.kind=NEW.kind
            WHERE restored.item=NEW.id AND restored.seq=NEW.seq AND discarded.seq=OLD.seq
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
