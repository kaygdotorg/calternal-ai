CREATE TABLE item_grants(
 id TEXT PRIMARY KEY,
 item_id TEXT NOT NULL,
 kind TEXT NOT NULL CHECK(kind IN ('notes','collections')),
 space_id TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
 created_by TEXT REFERENCES users(id) ON DELETE SET NULL,
 account_id TEXT REFERENCES users(id) ON DELETE CASCADE,
 group_id TEXT REFERENCES groups(id) ON DELETE CASCADE,
 permission INTEGER NOT NULL CHECK(permission IN (1,2)),
 link_hash BLOB UNIQUE,
 link_secret BLOB,
 label TEXT,
 expires_at INTEGER,
 CHECK(
   (link_hash IS NULL AND link_secret IS NULL AND label IS NULL AND expires_at IS NULL
     AND ((account_id IS NULL) <> (group_id IS NULL)))
   OR
   (link_hash IS NOT NULL AND link_secret IS NOT NULL AND label IS NOT NULL
     AND account_id IS NULL AND group_id IS NULL AND permission=1)
 )
) STRICT;
CREATE UNIQUE INDEX item_grant_account ON item_grants(item_id,account_id) WHERE account_id IS NOT NULL;
CREATE UNIQUE INDEX item_grant_group ON item_grants(item_id,group_id) WHERE group_id IS NOT NULL;
CREATE INDEX item_grant_item ON item_grants(item_id,id);
CREATE INDEX item_grant_recipient ON item_grants(account_id,kind,item_id);
CREATE INDEX item_grant_group_recipient ON item_grants(group_id,kind,item_id);
CREATE INDEX item_grant_space ON item_grants(space_id,item_id);
CREATE INDEX item_grant_creator ON item_grants(created_by);
CREATE INDEX item_link_expiry ON item_grants(expires_at) WHERE expires_at IS NOT NULL;
CREATE TABLE item_link_policies(
 space_id TEXT PRIMARY KEY REFERENCES spaces(id) ON DELETE CASCADE
) STRICT;

CREATE TABLE item_share_seen(
 account_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 space_id TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
 item_id TEXT NOT NULL,
 PRIMARY KEY(account_id,item_id)
) STRICT;
CREATE INDEX item_share_seen_space ON item_share_seen(space_id,item_id);
