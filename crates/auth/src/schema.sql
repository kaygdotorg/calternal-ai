-- auth.db, version 8: the current schema in one definition (no migration chain).
-- Accounts and their credentials, server roles, groups and invites (existing app
-- ADR 0058-0060), and spaces with their memberships and directory receipts.
-- Disposable preview data only: an auth.db at any other version is refused.

CREATE TABLE users (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    username    TEXT NOT NULL UNIQUE COLLATE NOCASE,
    created_at  INTEGER NOT NULL,
    role        TEXT NOT NULL DEFAULT 'member',
    disabled_at INTEGER,
    quota_bytes INTEGER CHECK (quota_bytes IS NULL OR quota_bytes >= 0)
);
CREATE TABLE passkeys (
    cred_id      BLOB PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id),
    passkey      TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    last_used_at INTEGER,
    name         TEXT NOT NULL DEFAULT 'Passkey'
);
CREATE INDEX passkeys_user ON passkeys(user_id);
CREATE TABLE setup_tokens (
    hash        BLOB PRIMARY KEY,
    purpose     TEXT NOT NULL CHECK (purpose IN ('first-owner', 'recover')),
    user_id     TEXT REFERENCES users(id),
    created_at  INTEGER NOT NULL,
    expires_at  INTEGER NOT NULL,
    used_at     INTEGER
);
CREATE TABLE installations (
    id                   TEXT PRIMARY KEY,
    user_id              TEXT NOT NULL REFERENCES users(id),
    kind                 TEXT NOT NULL CHECK (kind IN ('browser', 'native', 'cli', 'agent')),
    name                 TEXT NOT NULL,
    token_hash           BLOB NOT NULL UNIQUE,
    created_at           INTEGER NOT NULL,
    last_seen_at         INTEGER NOT NULL,
    expires_at           INTEGER,
    revoked_at           INTEGER,
    assertion_at         INTEGER,
    registration_session INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX installations_user ON installations(user_id);

-- Server-wide roles as capability sets (ADR 0058); the first person is admin.
CREATE TABLE roles (name TEXT PRIMARY KEY);
CREATE TABLE role_capabilities (
    role       TEXT NOT NULL REFERENCES roles(name),
    capability TEXT NOT NULL,
    PRIMARY KEY (role, capability)
);
INSERT INTO roles VALUES ('admin'), ('member');
INSERT INTO role_capabilities VALUES
    ('admin','people.invite'), ('admin','people.manage'),
    ('admin','groups.manage'), ('admin','server.settings');
CREATE TRIGGER user_role_insert BEFORE INSERT ON users
WHEN NOT EXISTS (SELECT 1 FROM roles WHERE name=NEW.role)
BEGIN SELECT RAISE(ABORT,'unknown role'); END;
CREATE TRIGGER user_role_update BEFORE UPDATE OF role ON users
WHEN NOT EXISTS (SELECT 1 FROM roles WHERE name=NEW.role)
BEGIN SELECT RAISE(ABORT,'unknown role'); END;
CREATE TRIGGER role_in_use BEFORE DELETE ON roles
WHEN EXISTS (SELECT 1 FROM users WHERE role=OLD.name)
BEGIN SELECT RAISE(ABORT,'role in use'); END;
CREATE TRIGGER first_person_admin AFTER INSERT ON users
WHEN (SELECT count(*) FROM users)=1
BEGIN UPDATE users SET role='admin' WHERE id=NEW.id; END;

-- Groups carry plan entitlements; an invite assigns its groups (ADR 0059, 0060).
CREATE TABLE groups (
    id     TEXT PRIMARY KEY,
    name   TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'local' CHECK (source IN ('local','oidc'))
);
CREATE TABLE group_members (
    group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    user_id  TEXT NOT NULL REFERENCES users(id),
    manager  INTEGER NOT NULL DEFAULT 0 CHECK (manager IN (0,1)),
    PRIMARY KEY (group_id, user_id)
);
CREATE INDEX group_people ON group_members(user_id, group_id);
CREATE TABLE invites (
    id             TEXT PRIMARY KEY,
    created_by     TEXT NOT NULL REFERENCES users(id),
    hash           BLOB NOT NULL UNIQUE,
    secret         BLOB NOT NULL,
    created_at     INTEGER NOT NULL,
    expires_at     INTEGER NOT NULL,
    used_at        INTEGER,
    revoked_at     INTEGER,
    quota_bytes    INTEGER CHECK (quota_bytes IS NULL OR quota_bytes >= 0),
    plan           TEXT,
    space_id       TEXT REFERENCES spaces(id) ON DELETE CASCADE,
    space_role     TEXT REFERENCES space_roles(name) CHECK (space_role <> 'owner')
);
CREATE INDEX invites_live ON invites(space_id, expires_at)
 WHERE used_at IS NULL AND revoked_at IS NULL;
CREATE INDEX invites_page ON invites(space_id, id);
CREATE INDEX invites_terminal ON invites(
 min(expires_at, coalesce(used_at, expires_at), coalesce(revoked_at, expires_at))
);
CREATE TABLE invite_groups (
    invite_id TEXT NOT NULL REFERENCES invites(id) ON DELETE CASCADE,
    group_id  TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    PRIMARY KEY (invite_id, group_id)
);

-- Spaces: roles are capability sets; the owner is the spaces row, not a membership role.
CREATE TABLE space_roles(name TEXT PRIMARY KEY) STRICT;
CREATE TABLE space_role_capabilities(
    role TEXT NOT NULL REFERENCES space_roles(name),
    capability TEXT NOT NULL,
    PRIMARY KEY(role,capability)
) STRICT;
INSERT INTO space_roles VALUES('reader'),('editor'),('manager'),('owner');
INSERT INTO space_role_capabilities VALUES
 ('reader','space.read'),
 ('editor','space.read'),('editor','content.write'),('editor','plugins.use'),
 ('manager','space.read'),('manager','content.write'),('manager','plugins.use'),
 ('manager','plugins.manage'),('manager','members.invite'),('manager','members.manage'),('manager','space.settings'),('manager','secrets.manage');
INSERT INTO space_role_capabilities SELECT 'owner',capability FROM space_role_capabilities WHERE role='manager';
INSERT INTO space_role_capabilities VALUES('owner','managers.manage'),('owner','space.transfer'),('owner','space.delete');
CREATE TABLE spaces(
 id TEXT PRIMARY KEY,
 name TEXT NOT NULL,
 owner_account TEXT NOT NULL REFERENCES users(id),
 created INTEGER NOT NULL,
 personal_for TEXT UNIQUE REFERENCES users(id),
 FOREIGN KEY(id,owner_account) REFERENCES space_memberships(space_id,account_id) DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE TABLE space_memberships(
 space_id TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
 account_id TEXT NOT NULL REFERENCES users(id),
 role TEXT NOT NULL REFERENCES space_roles(name) CHECK(role<>'owner'),
 PRIMARY KEY(space_id,account_id)
) STRICT;
CREATE INDEX spaces_account ON space_memberships(account_id,space_id);
CREATE TABLE space_group_memberships(
 space_id TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
 group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
 role TEXT NOT NULL REFERENCES space_roles(name) CHECK(role<>'owner'),
 PRIMARY KEY(space_id,group_id)
) STRICT;
CREATE INDEX spaces_group ON space_group_memberships(group_id,space_id);
-- This view expands current direct and group grants, without materialized
-- membership or a cache to invalidate when a grant changes.
CREATE VIEW space_account_roles AS
 SELECT m.space_id,m.account_id,
        CASE WHEN s.owner_account=m.account_id THEN 'owner' ELSE m.role END AS role
 FROM space_memberships m JOIN spaces s ON s.id=m.space_id
 JOIN users u ON u.id=m.account_id AND u.disabled_at IS NULL
 UNION ALL
 SELECT m.space_id,g.user_id,m.role
 FROM group_members g JOIN space_group_memberships m ON m.group_id=g.group_id
 JOIN users u ON u.id=g.user_id AND u.disabled_at IS NULL;

-- Receipts of directory changes (spaces.*): one per (acting account, scope,
-- request ID), where scope is the space, or the actor's own account for an
-- operation with no space. Kept for the receipt retention, then pruned.
CREATE TABLE space_receipts(
 actor TEXT NOT NULL REFERENCES users(id),
 scope TEXT NOT NULL,
 request_id TEXT NOT NULL,
 operation TEXT NOT NULL,
 fingerprint TEXT NOT NULL,
 result TEXT NOT NULL,
 at_ms INTEGER NOT NULL,
 PRIMARY KEY(actor,scope,request_id)
) STRICT;
CREATE INDEX space_receipts_age ON space_receipts(actor,at_ms);
-- Machine authority: who may run code where. A NULL owner is the operator's
-- pool. Codes store only their hash; expired codes are pruned after a day.
CREATE TABLE machine_codes(
 hash BLOB PRIMARY KEY,
 owner_account TEXT REFERENCES users(id) ON DELETE CASCADE,
 created_ms INTEGER NOT NULL,
 expires_ms INTEGER NOT NULL,
 used_ms INTEGER
) STRICT;
CREATE INDEX machine_codes_owner ON machine_codes(owner_account);
CREATE TABLE machines(
 id TEXT PRIMARY KEY,
 owner_account TEXT REFERENCES users(id) ON DELETE CASCADE,
 public_key BLOB NOT NULL,
 name TEXT NOT NULL,
 os TEXT NOT NULL,
 arch TEXT NOT NULL,
 enrolled_ms INTEGER NOT NULL,
 revoked_ms INTEGER,
 direct INTEGER NOT NULL DEFAULT 0 CHECK(direct IN (0,1)),
 CHECK(direct=0 OR owner_account IS NOT NULL)
) STRICT;
CREATE INDEX machines_owner ON machines(owner_account);
CREATE TABLE machine_pool_grants(
 account TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
 granted_ms INTEGER NOT NULL
) STRICT;
-- What the development seed created, so a restart returns the same fixture IDs and
-- never resurrects removed memberships or deleted spaces. Never read by requests.
CREATE TABLE development_directory(name TEXT PRIMARY KEY, id TEXT NOT NULL UNIQUE) STRICT;
CREATE TRIGGER space_identity_separate BEFORE INSERT ON spaces
WHEN EXISTS(SELECT 1 FROM users WHERE id=NEW.id)
BEGIN SELECT RAISE(ABORT,'space and account identities must differ'); END;
CREATE TRIGGER account_identity_separate BEFORE INSERT ON users
WHEN EXISTS(SELECT 1 FROM spaces WHERE id=NEW.id)
BEGIN SELECT RAISE(ABORT,'account and space identities must differ'); END;

-- Fingerprints only. No key bytes may enter auth.db or recovery. Tombstones
-- survive deletion so restoring an old ciphertext database cannot revive it.
CREATE TABLE owner_secret_keys(
 kind TEXT NOT NULL CHECK(kind IN ('account','space')),
 owner_id TEXT NOT NULL,
 fingerprint BLOB NOT NULL CHECK(length(fingerprint)=32),
 erasing INTEGER NOT NULL CHECK(erasing IN (0,1)),
 PRIMARY KEY(kind,owner_id)
) STRICT;
CREATE TRIGGER erase_space_secret_key AFTER DELETE ON spaces
BEGIN UPDATE owner_secret_keys SET erasing=1 WHERE kind='space' AND owner_id=OLD.id; END;
CREATE TRIGGER erase_account_secret_key AFTER DELETE ON users
BEGIN UPDATE owner_secret_keys SET erasing=1 WHERE kind='account' AND owner_id=OLD.id; END;
-- Bounded per-turn authority; no raw credential or provider login is stored.
CREATE TABLE agent_access(
 thread TEXT NOT NULL, job TEXT NOT NULL UNIQUE,
 account TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 space TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
 hash BLOB UNIQUE, scope TEXT NOT NULL CHECK(length(scope)<=65536),
 expires_ms INTEGER NOT NULL, run INTEGER NOT NULL,
 updated_ms INTEGER NOT NULL,
 PRIMARY KEY(account,space,thread)
) STRICT;
CREATE INDEX agent_access_account ON agent_access(account);
CREATE TABLE agent_approvals(
 id TEXT PRIMARY KEY, job TEXT NOT NULL,
 account TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 space TEXT NOT NULL, thread TEXT NOT NULL,
 app TEXT NOT NULL, write_access INTEGER NOT NULL,
 status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','answered','denied','once','this_thread','cancelled')),
 kind TEXT NOT NULL DEFAULT 'approval' CHECK(kind IN ('approval','question')),
 action TEXT NOT NULL CHECK(length(CAST(action AS BLOB))<=16384),
 action_hash TEXT CHECK(action_hash IS NULL OR length(action_hash)=64),
 decision_request TEXT, decision_value TEXT CHECK(length(CAST(decision_value AS BLOB))<=16384),
 resolved_ms INTEGER,
 claimed_id TEXT, claimed_hash TEXT, claimed_ms INTEGER,
 FOREIGN KEY(space) REFERENCES spaces(id) ON DELETE CASCADE
) STRICT;
CREATE INDEX agent_approvals_job ON agent_approvals(job);
CREATE INDEX agent_approvals_thread ON agent_approvals(space,thread);
CREATE INDEX agent_approvals_account ON agent_approvals(account,space,thread);
-- Membership changes and authority cleanup share the protected recovery commit.
CREATE TRIGGER erase_member_agent_authority AFTER DELETE ON space_memberships
WHEN NOT EXISTS (
 SELECT 1 FROM space_account_roles m JOIN space_role_capabilities c USING(role)
 WHERE m.account_id=OLD.account_id AND m.space_id=OLD.space_id AND c.capability='space.read'
)
BEGIN
 DELETE FROM agent_approvals WHERE account=OLD.account_id AND space=OLD.space_id;
 DELETE FROM agent_access WHERE account=OLD.account_id AND space=OLD.space_id;
END;
CREATE TRIGGER erase_unreadable_member_agent_authority AFTER UPDATE OF role ON space_memberships
WHEN NOT EXISTS (
 SELECT 1 FROM space_role_capabilities
 WHERE role=NEW.role AND capability='space.read'
 UNION ALL
 SELECT 1 FROM space_account_roles m JOIN space_role_capabilities c USING(role)
 WHERE m.account_id=NEW.account_id AND m.space_id=NEW.space_id AND c.capability='space.read'
)
BEGIN
 DELETE FROM agent_approvals WHERE account=NEW.account_id AND space=NEW.space_id;
 DELETE FROM agent_access WHERE account=NEW.account_id AND space=NEW.space_id;
END;
CREATE TRIGGER erase_group_grant_agent_authority AFTER DELETE ON space_group_memberships
BEGIN
 DELETE FROM agent_approvals WHERE space=OLD.space_id
  AND account IN (SELECT user_id FROM group_members WHERE group_id=OLD.group_id)
  AND NOT EXISTS (
   SELECT 1 FROM space_account_roles m JOIN space_role_capabilities c USING(role)
   WHERE m.account_id=agent_approvals.account AND m.space_id=OLD.space_id AND c.capability='space.read'
  );
 DELETE FROM agent_access WHERE space=OLD.space_id
  AND account IN (SELECT user_id FROM group_members WHERE group_id=OLD.group_id)
  AND NOT EXISTS (
   SELECT 1 FROM space_account_roles m JOIN space_role_capabilities c USING(role)
   WHERE m.account_id=agent_access.account AND m.space_id=OLD.space_id AND c.capability='space.read'
  );
END;
CREATE TRIGGER erase_group_member_agent_authority AFTER DELETE ON group_members
BEGIN
 DELETE FROM agent_approvals WHERE account=OLD.user_id
  AND space IN (SELECT space_id FROM space_group_memberships WHERE group_id=OLD.group_id)
  AND NOT EXISTS (
   SELECT 1 FROM space_account_roles m JOIN space_role_capabilities c USING(role)
   WHERE m.account_id=OLD.user_id AND m.space_id=agent_approvals.space AND c.capability='space.read'
  );
 DELETE FROM agent_access WHERE account=OLD.user_id
  AND space IN (SELECT space_id FROM space_group_memberships WHERE group_id=OLD.group_id)
  AND NOT EXISTS (
   SELECT 1 FROM space_account_roles m JOIN space_role_capabilities c USING(role)
   WHERE m.account_id=OLD.user_id AND m.space_id=agent_access.space AND c.capability='space.read'
  );
END;
CREATE TABLE agent_discards(
 account TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 thread TEXT NOT NULL, lease TEXT NOT NULL, authorized_ms INTEGER NOT NULL,
 PRIMARY KEY(account,thread)
) STRICT;
