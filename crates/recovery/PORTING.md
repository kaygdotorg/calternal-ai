# Source and adaptations

Reference: `/home/kayg/Developer/calternal`, `origin/dev` at
`ea57b84b9c9e40e8b67b7d1e1f1805915123b040`, inspected with git only.

- `docs/adr/0062-user-files-on-hdd-app-data-on-ssd.md:404–458`: implements
  protected security-before-ack records with space terminology. The old section
  says “Home”; this crate uses a protected installation directory in the SSD
  space tree. Namespace recovery, quotas/invites not yet exposed by this auth
  library, external UUID/root validation, and rebuilding empty state are outside
  this bounded crate. Pending-intent state loss refuses recovery rather than
  guessing a transaction decision. See PROTOCOL.md for the availability limit.
- `crates/auth/src/db.rs:413–452`: retains the ported worker's WAL/FULL transaction
  durability. Adds a hook around commits instead of importing old engine/server
  or plugin dependencies. No schema migration in the auth crate is changed;
  the optional coordinator owns its witness table.
- `crates/fsio/src/swap.rs:102–119,163–200`: inspected recoverable settings exchange;
  not copied because immutable version records have no displaced image to restore.
  Reuses this workspace's fsio mode-aware atomic writer and directory fsync instead.
- `crates/auth/src/setup.rs:324–399`, `passkeys.rs:201–233`, session revocation
  paths identified in port-map §6: keep the already-ported auth checks and insert
  the barrier at their commits. Autocommit passkey insertion/session revocation
  become explicit IMMEDIATE transactions so witness and mutation commit together.

The port-map §6 verifies no existing FO1 coordinator or tests exist. **Zero source
FO1 tests are copied or ignored.** New coordinator tests supplement the existing
ported fsio/auth suites; their results are reported separately. No source test was
weakened, skipped or renamed to imply FO1 coverage.

The reconstructed logical snapshot excludes SQLite/coordinator bookkeeping and
sharing delivery/reset state. Rebuild must install its own validated current
schema, never execute stored SQL, and force every sharing device through a fresh
authorized resync. Version 2 uses indexed transactional dirty-owner capture and
an immutable authenticated radix index bound by a fixed installation head.
Publication writes one delta intent and only affected owner records/path nodes;
post-publication reclamation deletes exact retired paths. This coordinator is a
new implementation of the inherited requirement, not source code ported from an
existing FO1 engine. No historical data is imported or migrated.
