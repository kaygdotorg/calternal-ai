-- One registered physical ordering per kind. Both directions use the same
-- B-tree. The kind writer already owns this space and its canonical metadata.
CREATE TABLE window_kind_orders (
    kind TEXT PRIMARY KEY REFERENCES kinds(kind),
    title_field TEXT NOT NULL,
    sort_field TEXT NOT NULL,
    parent_field TEXT
) STRICT, WITHOUT ROWID;

INSERT INTO window_kind_orders
SELECT kind,'title',
       CASE kind WHEN 'collections' THEN 'title' ELSE 'updated' END,
       CASE kind WHEN 'threads' THEN 'parent_thread' END
FROM kinds WHERE kind<>'notes'
  AND (kind IN ('collections','threads') OR json_extract(definition,'$[1].format')='markdown');

CREATE TRIGGER window_kind_register AFTER INSERT ON kinds
WHEN NEW.kind<>'notes'
  AND (NEW.kind IN ('collections','threads') OR json_extract(NEW.definition,'$[1].format')='markdown')
BEGIN
    INSERT INTO window_kind_orders VALUES(
        NEW.kind,'title',
        CASE NEW.kind WHEN 'collections' THEN 'title' ELSE 'updated' END,
        CASE NEW.kind WHEN 'threads' THEN 'parent_thread' END
    );
END;

-- Metadata field names are validated as simple keys on registration. Missing
-- and non-scalar values have one stable null-like key. Normalizing timestamps
-- and strings happens before seeking; skipped entries never consult a head.
CREATE VIEW window_generic_heads AS
SELECT i.id,i.kind,
       CASE
         WHEN o.sort_field='updated' THEN -coalesce(CAST(round(unixepoch(i.updated_at,'subsec')*1000) AS INTEGER),0)
         WHEN json_type(i.metadata,'$.'||o.sort_field)='text' THEN lower(json_extract(i.metadata,'$.'||o.sort_field))
         WHEN json_type(i.metadata,'$.'||o.sort_field) IN ('integer','real','true','false') THEN json_extract(i.metadata,'$.'||o.sort_field)
         ELSE X''
       END AS key,
       CASE WHEN json_type(i.metadata,'$.'||o.parent_field)='text'
                 AND length(json_extract(i.metadata,'$.'||o.parent_field))=36
            THEN lower(json_extract(i.metadata,'$.'||o.parent_field)) END AS parent
FROM items i JOIN window_kind_orders o ON o.kind=i.kind
WHERE i.mat_state NOT IN ('purging','purged') AND substr(i.path,1,7)<>'.trash/';

CREATE VIEW window_generic_candidates AS
SELECT id,kind,kind AS collection,'kind' AS audience,key FROM window_generic_heads
UNION ALL
SELECT id,kind,kind||':'||parent,'child',key FROM window_generic_heads WHERE parent IS NOT NULL;

INSERT INTO window_rows(collection,audience,sort_key,item)
SELECT collection,audience,key,id FROM window_generic_candidates;

CREATE TRIGGER window_generic_insert AFTER INSERT ON items
WHEN NEW.kind<>'notes'
BEGIN
    INSERT INTO window_rows(collection,audience,sort_key,item)
    SELECT collection,audience,key,id FROM window_generic_candidates WHERE id=NEW.id;
END;

CREATE TRIGGER window_generic_edit AFTER UPDATE OF revision,metadata,path,mat_state,updated_at ON items
WHEN NEW.kind<>'notes' AND (
    OLD.revision<>NEW.revision OR OLD.metadata<>NEW.metadata OR OLD.updated_at<>NEW.updated_at
    OR (substr(OLD.path,1,7)='.trash/')<>(substr(NEW.path,1,7)='.trash/')
    OR (OLD.mat_state IN ('purging','purged'))<>(NEW.mat_state IN ('purging','purged'))
)
BEGIN
    DELETE FROM window_rows WHERE item=NEW.id AND audience IN ('kind','child')
    AND (collection,audience) NOT IN (SELECT collection,audience FROM window_generic_candidates WHERE id=NEW.id);

    UPDATE window_rows SET sort_key=p.key
    FROM window_generic_candidates p
    WHERE item=NEW.id AND p.id=NEW.id AND (window_rows.collection,window_rows.audience)=(p.collection,p.audience);

    INSERT INTO window_rows(collection,audience,sort_key,item)
    SELECT p.collection,p.audience,p.key,p.id FROM window_generic_candidates p
    WHERE p.id=NEW.id AND NOT EXISTS(
        SELECT 1 FROM window_rows w WHERE (w.item,w.collection,w.audience)=(p.id,p.collection,p.audience)
    );
END;

CREATE TRIGGER window_generic_delete BEFORE DELETE ON items WHEN OLD.kind<>'notes'
BEGIN DELETE FROM window_rows WHERE item=OLD.id AND audience IN ('kind','child'); END;

-- Trash is an exclusive Notes projection. A trashed head has no live keys.
CREATE VIEW window_trash_notes AS
SELECT id,-coalesce(CAST(round(unixepoch(updated_at,'subsec')*1000) AS INTEGER),0) AS key
FROM items WHERE kind='notes' AND substr(path,1,7)='.trash/' AND mat_state NOT IN ('purging','purged');

INSERT INTO window_rows
SELECT 'notes','trash',key,id FROM window_trash_notes;

CREATE TRIGGER window_notes_trash_insert AFTER INSERT ON items
WHEN NEW.kind='notes' AND substr(NEW.path,1,7)='.trash/'
BEGIN
    INSERT INTO window_rows SELECT 'notes','trash',key,id FROM window_trash_notes WHERE id=NEW.id;
END;

CREATE TRIGGER window_notes_trash_edit AFTER UPDATE OF revision,metadata,path,mat_state,updated_at ON items
WHEN NEW.kind='notes' AND (substr(OLD.path,1,7)='.trash/' OR substr(NEW.path,1,7)='.trash/')
AND (OLD.revision<>NEW.revision OR OLD.metadata<>NEW.metadata OR OLD.updated_at<>NEW.updated_at
     OR (substr(OLD.path,1,7)='.trash/')<>(substr(NEW.path,1,7)='.trash/')
     OR (OLD.mat_state IN ('purging','purged'))<>(NEW.mat_state IN ('purging','purged')))
BEGIN
    DELETE FROM window_rows WHERE (item,collection,audience)=(NEW.id,'notes','trash')
    AND NOT EXISTS(SELECT 1 FROM window_trash_notes WHERE id=NEW.id);
    INSERT INTO window_rows SELECT 'notes','trash',key,id FROM window_trash_notes WHERE id=NEW.id
    ON CONFLICT(item,collection,audience) DO UPDATE SET sort_key=excluded.sort_key;
END;

-- Trash and live placement can change in either SQLite trigger order. The
-- one, exclusive Trash key does not reserve any of the live fan-out budget.
DROP TRIGGER window_row_limit;
CREATE TRIGGER window_row_limit BEFORE INSERT ON window_rows
WHEN NEW.audience<>'trash'
  AND (SELECT count(*) FROM window_rows WHERE item=NEW.item AND audience<>'trash')>=32
BEGIN SELECT RAISE(ABORT, 'effective_scope_limit_exceeded'); END;

-- Each manual collection has one published order. A member-only newest
-- projection can be provisioned without adding a second key per affected scope.
CREATE TABLE window_collection_newest (
    collection TEXT PRIMARY KEY REFERENCES items(id) ON DELETE CASCADE
) STRICT, WITHOUT ROWID;

CREATE VIEW window_note_candidates AS
SELECT id,'notes' AS collection,'space' AS audience,newest AS key FROM window_live_notes
UNION ALL
SELECT n.id,m.collection_id,
       CASE WHEN o.collection IS NULL THEN 'explicit' ELSE 'explicit_newest' END,
       CASE WHEN o.collection IS NULL THEN n.title ELSE n.newest END
FROM note_collection_members m JOIN window_live_notes n ON n.id=m.note_id
LEFT JOIN window_collection_newest o ON o.collection=m.collection_id;

DROP TRIGGER window_note_insert;
CREATE TRIGGER window_note_insert AFTER INSERT ON items WHEN NEW.kind='notes' BEGIN
    INSERT INTO window_rows SELECT collection,audience,key,id FROM window_note_candidates WHERE id=NEW.id;
END;

DROP TRIGGER window_note_restore;
CREATE TRIGGER window_note_restore AFTER UPDATE OF path,mat_state ON items
WHEN NEW.kind='notes'
    AND (OLD.mat_state IN ('purging','purged') OR substr(OLD.path,1,7)='.trash/')
    AND NEW.mat_state NOT IN ('purging','purged') AND substr(NEW.path,1,7)<>'.trash/'
BEGIN
    INSERT INTO window_rows SELECT collection,audience,key,id FROM window_note_candidates WHERE id=NEW.id;
END;

DROP TRIGGER window_note_remove;
CREATE TRIGGER window_note_remove AFTER UPDATE OF path,mat_state ON items
WHEN NEW.kind='notes'
    AND OLD.mat_state NOT IN ('purging','purged') AND substr(OLD.path,1,7)<>'.trash/'
    AND (NEW.mat_state IN ('purging','purged') OR substr(NEW.path,1,7)='.trash/')
BEGIN DELETE FROM window_rows WHERE item=NEW.id AND audience<>'trash'; END;

DROP TRIGGER window_note_edit;
CREATE TRIGGER window_note_edit AFTER UPDATE OF revision,updated_at,metadata ON items
WHEN NEW.kind='notes' AND (OLD.revision<>NEW.revision OR OLD.updated_at<>NEW.updated_at OR OLD.metadata<>NEW.metadata)
    AND OLD.mat_state NOT IN ('purging','purged') AND substr(OLD.path,1,7)<>'.trash/'
    AND NEW.mat_state NOT IN ('purging','purged') AND substr(NEW.path,1,7)<>'.trash/'
BEGIN
    UPDATE window_rows SET sort_key=n.newest
    FROM window_live_notes n WHERE item=NEW.id AND n.id=NEW.id AND audience IN ('space','explicit_newest');
    UPDATE window_rows SET sort_key=n.title
    FROM window_live_notes n WHERE item=NEW.id AND n.id=NEW.id AND audience='explicit'
        AND (CASE WHEN json_type(OLD.metadata,'$.title')='text' THEN json_extract(OLD.metadata,'$.title') ELSE '' END)
            <>(CASE WHEN json_type(NEW.metadata,'$.title')='text' THEN json_extract(NEW.metadata,'$.title') ELSE '' END);
END;

DROP TRIGGER window_member_insert;
CREATE TRIGGER window_member_insert AFTER INSERT ON note_collection_members BEGIN
    INSERT INTO window_rows
    SELECT collection,audience,key,id FROM window_note_candidates WHERE id=NEW.note_id AND collection=NEW.collection_id;
END;

DROP TRIGGER window_member_delete;
CREATE TRIGGER window_member_delete AFTER DELETE ON note_collection_members BEGIN
    DELETE FROM window_rows WHERE (item,collection)=(OLD.note_id,OLD.collection_id);
END;

CREATE TRIGGER window_collection_order_forget AFTER UPDATE OF mat_state ON items
WHEN NEW.kind='collections' AND NEW.mat_state IN ('purging','purged')
BEGIN DELETE FROM window_collection_newest WHERE collection=NEW.id; END;
