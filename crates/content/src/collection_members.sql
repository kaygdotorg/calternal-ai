CREATE TABLE note_collection_members(
 note_id TEXT NOT NULL,
 collection_id TEXT NOT NULL REFERENCES items(id),
 PRIMARY KEY(note_id,collection_id)
) STRICT;
CREATE INDEX collection_members_by_collection ON note_collection_members(collection_id,note_id);

INSERT OR IGNORE INTO note_collection_members(note_id,collection_id)
SELECT lower(member.value), item.id
FROM items item CROSS JOIN json_each(item.metadata,'$.members') member
WHERE item.kind='collections' AND json_extract(item.metadata,'$.item_kind')='notes'
  AND json_extract(item.metadata,'$.standard_key') IS NULL
  AND item.mat_state NOT IN ('purged','purging') AND substr(item.path,1,7)<>'.trash/'
  AND member.type='text';

CREATE TRIGGER collection_members_insert AFTER INSERT ON items
WHEN NEW.kind='collections' AND json_extract(NEW.metadata,'$.item_kind')='notes'
  AND json_extract(NEW.metadata,'$.standard_key') IS NULL
  AND NEW.mat_state NOT IN ('purged','purging') AND substr(NEW.path,1,7)<>'.trash/'
BEGIN
 INSERT OR IGNORE INTO note_collection_members(note_id,collection_id)
 SELECT lower(value), NEW.id FROM json_each(NEW.metadata,'$.members') WHERE type='text';
END;

CREATE TRIGGER collection_members_update AFTER UPDATE OF metadata,path,mat_state ON items
WHEN OLD.kind='collections' OR NEW.kind='collections'
BEGIN
 DELETE FROM note_collection_members WHERE collection_id=OLD.id;
 INSERT OR IGNORE INTO note_collection_members(note_id,collection_id)
 SELECT lower(value), NEW.id FROM json_each(NEW.metadata,'$.members')
 WHERE type='text' AND NEW.kind='collections' AND json_extract(NEW.metadata,'$.item_kind')='notes'
  AND json_extract(NEW.metadata,'$.standard_key') IS NULL
  AND NEW.mat_state NOT IN ('purged','purging') AND substr(NEW.path,1,7)<>'.trash/';
END;

CREATE TRIGGER collection_members_delete BEFORE DELETE ON items
WHEN OLD.kind='collections'
BEGIN DELETE FROM note_collection_members WHERE collection_id=OLD.id; END;
