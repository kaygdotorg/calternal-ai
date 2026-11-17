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
CREATE TABLE space_receipts(
 actor TEXT NOT NULL REFERENCES users(id),
 request_id TEXT NOT NULL,
 operation TEXT NOT NULL,
 fingerprint TEXT NOT NULL,
 result TEXT NOT NULL,
 PRIMARY KEY(actor,request_id)
) STRICT;
-- Explicit development-only bindings survive record deletion, preventing reseeding
-- from resurrecting removed memberships or deleted spaces on restart.
CREATE TABLE development_directory(name TEXT PRIMARY KEY, id TEXT NOT NULL UNIQUE) STRICT;

CREATE TRIGGER space_identity_separate BEFORE INSERT ON spaces
WHEN EXISTS(SELECT 1 FROM users WHERE id=NEW.id)
BEGIN SELECT RAISE(ABORT,'space and account identities must differ'); END;
CREATE TRIGGER account_identity_separate BEFORE INSERT ON users
WHEN EXISTS(SELECT 1 FROM spaces WHERE id=NEW.id)
BEGIN SELECT RAISE(ABORT,'account and space identities must differ'); END;
