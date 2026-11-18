CREATE TABLE space_resource_groups(
    space_id TEXT PRIMARY KEY REFERENCES spaces(id) ON DELETE CASCADE,
    group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE
) STRICT, WITHOUT ROWID;
CREATE INDEX space_resource_groups_by_group ON space_resource_groups(group_id);

CREATE TRIGGER resource_group_shared_space BEFORE INSERT ON space_resource_groups
WHEN EXISTS(SELECT 1 FROM spaces WHERE id=NEW.space_id AND personal_for IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'personal spaces use their account'); END;
