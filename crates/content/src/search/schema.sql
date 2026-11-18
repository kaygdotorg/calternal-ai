-- This database is derived. A schema/manifest/journal-epoch mismatch resets
-- its contents. Readers see only an entirely caught-up generation.
CREATE TABLE IF NOT EXISTS search_configuration(
  id INTEGER PRIMARY KEY CHECK(id=1), kinds TEXT NOT NULL,
  space TEXT NOT NULL, epoch TEXT NOT NULL, generation TEXT NOT NULL, watermark INTEGER NOT NULL,
  scan_after INTEGER, ready INTEGER NOT NULL
) STRICT;
CREATE TABLE IF NOT EXISTS search_docs(
  rowid INTEGER PRIMARY KEY, id TEXT NOT NULL UNIQUE, kind TEXT NOT NULL,
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
