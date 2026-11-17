-- auth.db, version 1: the current schema in one definition (no migration chain).
-- Accounts and their credentials, server roles, groups and invites (existing app
-- ADR 0058-0060), and spaces with their memberships and directory receipts.
-- Disposable preview data only: an auth.db at any other version is refused.

CREATE TABLE users (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    username    TEXT NOT NULL UNIQUE COLLATE NOCASE,
    created_at  INTEGER NOT NULL,
    role        TEXT NOT NULL DEFAULT 'member',
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
    lookup_version INTEGER NOT NULL DEFAULT 0
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
 ('manager','plugins.manage'),('manager','members.invite'),('manager','members.manage'),('manager','space.settings');
INSERT INTO space_role_capabilities SELECT 'owner',capability FROM space_role_capabilities WHERE role='manager';
INSERT INTO space_role_capabilities VALUES('owner','managers.manage'),('owner','space.transfer'),('owner','space.delete');
CREATE TABLE spaces(
 id TEXT PRIMARY KEY,
 name TEXT NOT NULL,
 owner_account TEXT NOT NULL REFERENCES users(id),
 created INTEGER NOT NULL,
 FOREIGN KEY(id,owner_account) REFERENCES space_memberships(space_id,account_id) DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE TABLE space_memberships(
 space_id TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
 account_id TEXT NOT NULL REFERENCES users(id),
 role TEXT NOT NULL REFERENCES space_roles(name) CHECK(role<>'owner'),
 PRIMARY KEY(space_id,account_id)
) STRICT;
CREATE INDEX spaces_account ON space_memberships(account_id,space_id);
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
 revoked_ms INTEGER
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
