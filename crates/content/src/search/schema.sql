-- The existing space write transaction advances both revision and projection.
-- This indexes at most 250,000 item slots and the first 8,192 body scalars per
-- live indexed item. The fold key and FTS text come from the current revision.
CREATE TABLE IF NOT EXISTS search_configuration(
  id INTEGER PRIMARY KEY CHECK(id=1), kinds TEXT NOT NULL
) STRICT;
CREATE TABLE IF NOT EXISTS search_docs(
  rowid INTEGER PRIMARY KEY, id TEXT NOT NULL, kind TEXT NOT NULL,
  title TEXT NOT NULL, title_key TEXT NOT NULL
) STRICT;
CREATE INDEX IF NOT EXISTS search_prefix ON search_docs(title_key,id);
CREATE INDEX IF NOT EXISTS search_kind_prefix ON search_docs(kind,title_key,id);
CREATE VIRTUAL TABLE IF NOT EXISTS search_text USING fts5(
  title, body, tokenize='unicode61 remove_diacritics 2', prefix='2 3'
);
CREATE VIRTUAL TABLE IF NOT EXISTS search_title_grams USING fts5(
  grams, content='', contentless_delete=1
);
CREATE VIEW IF NOT EXISTS search_heads AS
  SELECT i.rowid,i.id,i.kind,
         replace(replace(substr(i.metadata->>'title',1,256),char(1),''),char(2),'') AS title,
         search_fold(substr(i.metadata->>'title',1,256)) AS title_key
  FROM items i WHERE i.rowid<=250000 AND substr(i.path,1,7)<>'.trash/'
    AND i.mat_state NOT IN ('purging','purged')
    AND coalesce(i.metadata->>'deleted',0)<>1
    AND json_type(i.metadata,'$.title')='text'
    AND i.kind IN (SELECT value FROM search_configuration, json_each(kinds));
CREATE VIEW IF NOT EXISTS search_texts AS
  SELECT d.rowid,d.title,search_body(coalesce(substr(v.bytes,1,65536),x''))
  FROM search_docs d JOIN items i ON i.rowid=d.rowid
  JOIN versions v ON v.item=i.id AND v.seq=i.seq;
CREATE TRIGGER IF NOT EXISTS search_version AFTER INSERT ON versions
WHEN EXISTS (SELECT 1 FROM items i WHERE i.id=new.item AND i.seq=new.seq
             AND i.kind IN (SELECT value FROM search_configuration, json_each(kinds)))
BEGIN
  DELETE FROM search_text WHERE rowid=(SELECT rowid FROM items WHERE id=new.item);
  DELETE FROM search_title_grams WHERE rowid=(SELECT rowid FROM items WHERE id=new.item);
  DELETE FROM search_docs WHERE rowid=(SELECT rowid FROM items WHERE id=new.item);
  INSERT INTO search_docs SELECT * FROM search_heads WHERE id=new.item;
  INSERT INTO search_text(rowid,title,body) SELECT * FROM search_texts WHERE rowid=(SELECT rowid FROM items WHERE id=new.item);
  INSERT INTO search_title_grams(rowid,grams) SELECT rowid,search_grams(title_key) FROM search_docs WHERE rowid=(SELECT rowid FROM items WHERE id=new.item);
END;
