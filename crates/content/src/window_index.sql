-- Authoritative configuration and scope admission. Covering rows and totals
-- are published in the rebuildable window sidecar by the journal worker.
CREATE TABLE window_configuration (revision INTEGER NOT NULL) STRICT;
INSERT INTO window_configuration VALUES (0);

-- This projection derives its sort only from fields available to the
-- corresponding audience. Native notes.list uses the existing newest order.
CREATE VIEW window_live_notes AS
SELECT id,
       -coalesce(CAST(round(unixepoch(updated_at,'subsec')*1000) AS INTEGER),0) AS newest,
       CASE WHEN json_type(metadata,'$.title')='text' THEN lower(json_extract(metadata,'$.title')) ELSE '' END AS title
FROM items
WHERE kind='notes' AND mat_state NOT IN ('purging','purged') AND substr(path,1,7)<>'.trash/';

-- Reserve room for the implicit space projection even when the note is
-- currently trashed or the stored reference has no live head.
CREATE TRIGGER window_member_limit BEFORE INSERT ON note_collection_members
WHEN NOT EXISTS(SELECT 1 FROM note_collection_members WHERE (note_id,collection_id)=(NEW.note_id,NEW.collection_id))
    AND EXISTS(SELECT 1 FROM note_collection_members WHERE note_id=NEW.note_id LIMIT 1 OFFSET 30)
BEGIN SELECT RAISE(ABORT, 'effective_scope_limit_exceeded'); END;

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

-- Trash is an exclusive Notes projection. A trashed head has no live keys.
CREATE VIEW window_trash_notes AS
SELECT id,-coalesce(CAST(round(unixepoch(updated_at,'subsec')*1000) AS INTEGER),0) AS key
FROM items WHERE kind='notes' AND substr(path,1,7)='.trash/' AND mat_state NOT IN ('purging','purged');

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

CREATE TRIGGER window_collection_order_forget AFTER UPDATE OF mat_state ON items
WHEN NEW.kind='collections' AND NEW.mat_state IN ('purging','purged')
BEGIN DELETE FROM window_collection_newest WHERE collection=NEW.id; END;

-- A definition change has no journal event. Its generation is written only
-- when an order changes and fences both publication and window delivery.
CREATE TRIGGER window_kind_config_update AFTER UPDATE ON window_kind_orders
BEGIN UPDATE window_configuration SET revision=revision+1; END;
CREATE TRIGGER window_collection_config_insert AFTER INSERT ON window_collection_newest
BEGIN UPDATE window_configuration SET revision=revision+1; END;
CREATE TRIGGER window_collection_config_delete AFTER DELETE ON window_collection_newest
BEGIN UPDATE window_configuration SET revision=revision+1; END;
