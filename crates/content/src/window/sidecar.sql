CREATE TABLE window_rows(
    collection TEXT NOT NULL,
    audience TEXT NOT NULL,
    sort_key ANY NOT NULL,
    item TEXT NOT NULL,
    visible TEXT NOT NULL,
    PRIMARY KEY(item,collection,audience)
) STRICT;
CREATE INDEX window_cover ON window_rows(collection,audience,sort_key,item);
CREATE INDEX window_members ON window_rows(collection,item)
WHERE audience IN ('explicit','explicit_newest');
CREATE TABLE window_skipped(item TEXT PRIMARY KEY, reason TEXT NOT NULL) STRICT, WITHOUT ROWID;

CREATE TABLE window_scopes(
    collection TEXT NOT NULL,
    audience TEXT NOT NULL,
    total INTEGER NOT NULL CHECK(total>=0),
    version TEXT NOT NULL,
    PRIMARY KEY(collection,audience)
) STRICT, WITHOUT ROWID;

CREATE TRIGGER window_row_insert AFTER INSERT ON window_rows BEGIN
    INSERT INTO window_scopes VALUES(NEW.collection,NEW.audience,1,lower(hex(randomblob(16))))
    ON CONFLICT(collection,audience)
    DO UPDATE SET total=total+1,version=excluded.version;
END;

CREATE TRIGGER window_row_delete AFTER DELETE ON window_rows BEGIN
    UPDATE window_scopes SET total=total-1,version=lower(hex(randomblob(16)))
    WHERE (collection,audience)=(OLD.collection,OLD.audience);
    DELETE FROM window_scopes WHERE (collection,audience)=(OLD.collection,OLD.audience) AND total=0;
END;

CREATE TRIGGER window_row_update AFTER UPDATE OF sort_key,visible ON window_rows BEGIN
    UPDATE window_scopes SET version=lower(hex(randomblob(16)))
    WHERE (collection,audience)=(NEW.collection,NEW.audience);
END;


CREATE TABLE window_publication (
    id INTEGER PRIMARY KEY CHECK(id=1),
    epoch TEXT NOT NULL, sequence INTEGER NOT NULL, config INTEGER NOT NULL,
    scan_after TEXT, ready INTEGER NOT NULL CHECK(ready IN (0,1)), member_after TEXT
) STRICT;
