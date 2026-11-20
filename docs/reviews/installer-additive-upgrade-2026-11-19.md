# Installer additive upgrade from c54f0aed044d

The running release shipped the package catalogue at schema 3. Adding
`install_order` to its original `CREATE TABLE IF NOT EXISTS` had no
effect on that table when an existing installation restarted. The
shared SQLite migration runner now adds the nullable column in schema 4
before catalogue startup or release admission. The established
`COALESCE(install_order, revision), revision` query preserves the order
of existing packages. The migration performs no backfill or row update.

## Schema inventory

| Change since the running release | Existing database path |
| --- | --- |
| Installer · `installed_plugins.install_order` | The shared package-store schema runs `ALTER TABLE installed_plugins ADD COLUMN install_order INTEGER` at version 4 in the same transaction as the version update. Fresh stores apply the original schema and the same versioned steps. |
| Installer · other tables, columns and indexes | No additions. `installed_plugins`, `retained_plugins` and its ID index, `plugin_seed`, `plugin_draft_counter` and the schema-3 `provider_runtime` table already ship. Installer v2 stores archives in the existing table. |
| Separate microVM merge · `space_runtime_locations`, `space_runtime_checkpoints`, `space_runtime_erasures` | The machine-store version 10 step creates all three and the account-location and erasure-delivery indexes. Existing machine rows remain under their previous schema. |

The old-schema regression creates a version-3 database without the new
column and inserts an installed v1 archive. It opens the current
catalogue, reads the installed package, commits a second package through
the normal installer writer, reopens, and checks that the old archive,
checksum, revision, approval and enablement survived with a null
`install_order`.

The production-style release-pair check takes the previous and candidate
image tags through `CALTERNAL_UPGRADE_PREVIOUS_IMAGE` and
`CALTERNAL_UPGRADE_CANDIDATE_IMAGE` in
[`scripts/verify-plugin-workers.sh`](../../scripts/verify-plugin-workers.sh).
It writes image-bound evidence after both releases have served data from
the fixture. The previous server understands the additive column and
v1 archives. A stored v2 archive still requires the new reader. A
rollback to the previous server must account for any v2 archives created
after upgrade. The schema proof does not qualify that format change.

## Simplification opportunities

The startup and release paths now share the same versioned schema.
Keep the release-pair gate alongside the installer tests so a fresh
database pass cannot serve as evidence for an existing database.
