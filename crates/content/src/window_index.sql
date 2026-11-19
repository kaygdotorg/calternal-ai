-- The audience key identifies a row set, not a grant. All current grants to
-- an explicit Notes collection authorize the same live member set.
CREATE TABLE window_rows(
    collection TEXT NOT NULL,
    audience TEXT NOT NULL,
    sort_key ANY NOT NULL,
    item TEXT NOT NULL REFERENCES items(id),
    PRIMARY KEY(item,collection,audience)
) STRICT;
CREATE INDEX window_cover ON window_rows(collection,audience,sort_key,item);

CREATE TABLE window_scopes(
    collection TEXT NOT NULL,
    audience TEXT NOT NULL,
    total INTEGER NOT NULL CHECK(total>=0),
    version TEXT NOT NULL,
    PRIMARY KEY(collection,audience)
) STRICT, WITHOUT ROWID;

-- This projection derives its sort only from fields available to the
-- corresponding audience. Native notes.list uses the existing newest order.
CREATE VIEW window_live_notes AS
SELECT id,
       -coalesce(CAST(round(unixepoch(updated_at,'subsec')*1000) AS INTEGER),0) AS newest,
       CASE WHEN json_type(metadata,'$.title')='text' THEN lower(json_extract(metadata,'$.title')) ELSE '' END AS title
FROM items
WHERE kind='notes' AND mat_state NOT IN ('purging','purged') AND substr(path,1,7)<>'.trash/';

CREATE TRIGGER window_row_limit BEFORE INSERT ON window_rows
WHEN (SELECT count(*) FROM window_rows WHERE item=NEW.item)>=32
BEGIN SELECT RAISE(ABORT, 'effective_scope_limit_exceeded'); END;

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

CREATE TRIGGER window_row_update AFTER UPDATE OF sort_key ON window_rows BEGIN
    UPDATE window_scopes SET version=lower(hex(randomblob(16)))
    WHERE (collection,audience)=(NEW.collection,NEW.audience);
END;

INSERT INTO window_rows
SELECT 'notes','space',newest,id FROM window_live_notes
UNION ALL
SELECT m.collection_id,'explicit',n.title,n.id
FROM note_collection_members m JOIN window_live_notes n ON n.id=m.note_id;

CREATE TRIGGER window_note_insert AFTER INSERT ON items WHEN NEW.kind='notes' BEGIN
    INSERT INTO window_rows
    SELECT 'notes','space',newest,id FROM window_live_notes WHERE id=NEW.id
    UNION ALL
    SELECT m.collection_id,'explicit',n.title,n.id
    FROM note_collection_members m JOIN window_live_notes n ON n.id=m.note_id
    WHERE m.note_id=NEW.id;
END;

CREATE TRIGGER window_note_remove AFTER UPDATE OF path,mat_state ON items
WHEN NEW.kind='notes'
    AND OLD.mat_state NOT IN ('purging','purged') AND substr(OLD.path,1,7)<>'.trash/'
    AND (NEW.mat_state IN ('purging','purged') OR substr(NEW.path,1,7)='.trash/')
BEGIN DELETE FROM window_rows WHERE item=NEW.id; END;

CREATE TRIGGER window_note_restore AFTER UPDATE OF path,mat_state ON items
WHEN NEW.kind='notes'
    AND (OLD.mat_state IN ('purging','purged') OR substr(OLD.path,1,7)='.trash/')
    AND NEW.mat_state NOT IN ('purging','purged') AND substr(NEW.path,1,7)<>'.trash/'
BEGIN
    INSERT INTO window_rows
    SELECT 'notes','space',newest,id FROM window_live_notes WHERE id=NEW.id
    UNION ALL
    SELECT m.collection_id,'explicit',n.title,n.id
    FROM note_collection_members m JOIN window_live_notes n ON n.id=m.note_id
    WHERE m.note_id=NEW.id;
END;

CREATE TRIGGER window_note_edit AFTER UPDATE OF revision,updated_at,metadata ON items
WHEN NEW.kind='notes' AND (OLD.revision<>NEW.revision OR OLD.updated_at<>NEW.updated_at OR OLD.metadata<>NEW.metadata)
    AND OLD.mat_state NOT IN ('purging','purged') AND substr(OLD.path,1,7)<>'.trash/'
    AND NEW.mat_state NOT IN ('purging','purged') AND substr(NEW.path,1,7)<>'.trash/'
BEGIN
    UPDATE window_rows SET sort_key=n.newest
    FROM window_live_notes n WHERE item=NEW.id AND n.id=NEW.id AND audience='space';
    -- Explicit projections expose only titles. A body or hidden-metadata edit
    -- cannot change their order or version.
    UPDATE window_rows SET sort_key=n.title
    FROM window_live_notes n WHERE item=NEW.id AND n.id=NEW.id AND audience='explicit'
        AND (CASE WHEN json_type(OLD.metadata,'$.title')='text' THEN json_extract(OLD.metadata,'$.title') ELSE '' END)
            <>(CASE WHEN json_type(NEW.metadata,'$.title')='text' THEN json_extract(NEW.metadata,'$.title') ELSE '' END);
END;

CREATE TRIGGER window_note_delete BEFORE DELETE ON items WHEN OLD.kind='notes'
BEGIN DELETE FROM window_rows WHERE item=OLD.id; END;

-- Reserve room for the implicit space projection even when the note is
-- currently trashed or the stored reference has no live head.
CREATE TRIGGER window_member_limit BEFORE INSERT ON note_collection_members
WHEN NOT EXISTS(SELECT 1 FROM note_collection_members WHERE (note_id,collection_id)=(NEW.note_id,NEW.collection_id))
    AND (SELECT count(*) FROM note_collection_members WHERE note_id=NEW.note_id)>=31
BEGIN SELECT RAISE(ABORT, 'effective_scope_limit_exceeded'); END;

CREATE TRIGGER window_member_insert AFTER INSERT ON note_collection_members BEGIN
    INSERT INTO window_rows
    SELECT NEW.collection_id,'explicit',title,id FROM window_live_notes WHERE id=NEW.note_id;
END;

CREATE TRIGGER window_member_delete AFTER DELETE ON note_collection_members BEGIN
    DELETE FROM window_rows WHERE (item,collection,audience)=(OLD.note_id,OLD.collection_id,'explicit');
END;

-- Replace the existing membership maintenance with a delta. Unchanged members
-- retain their keys and counts, including when only the collection title changes.
DROP TRIGGER collection_members_update;
CREATE TRIGGER collection_members_update AFTER UPDATE OF metadata,path,mat_state ON items
WHEN (OLD.kind='collections' OR NEW.kind='collections')
    AND (OLD.metadata<>NEW.metadata OR substr(OLD.path,1,7)<>substr(NEW.path,1,7)
         OR (OLD.mat_state IN ('purging','purged'))<>(NEW.mat_state IN ('purging','purged')))
BEGIN
    DELETE FROM note_collection_members WHERE collection_id=OLD.id AND (
        NEW.kind<>'collections' OR json_extract(NEW.metadata,'$.item_kind') IS NOT 'notes'
        OR json_extract(NEW.metadata,'$.standard_key') IS NOT NULL
        OR NEW.mat_state IN ('purged','purging') OR substr(NEW.path,1,7)='.trash/'
        OR note_id NOT IN (SELECT lower(value) FROM json_each(NEW.metadata,'$.members') WHERE type='text')
    );
    INSERT INTO note_collection_members(note_id,collection_id)
    SELECT DISTINCT lower(value), NEW.id FROM json_each(NEW.metadata,'$.members')
    WHERE type='text' AND NEW.kind='collections' AND json_extract(NEW.metadata,'$.item_kind')='notes'
        AND json_extract(NEW.metadata,'$.standard_key') IS NULL
        AND NEW.mat_state NOT IN ('purged','purging') AND substr(NEW.path,1,7)<>'.trash/'
        AND lower(value) NOT IN (SELECT note_id FROM note_collection_members WHERE collection_id=NEW.id);
END;
