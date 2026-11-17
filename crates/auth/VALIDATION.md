# Account library port validation

Step 4 validated in the shared `/home/kayg/Developer/Calternal-AI` checkout.
All account/database fixtures used disposable tempfile directories. No real
Calternal data, `.dev-data`, port 3210, source checkout build/write, package
installation, commit, push or agent spawn was used. Other lanes' files were
not edited. Cargo updated the shared lockfile to resolve the new dependencies.

The initial OpenSSL system-package preflight was superseded by the parent
lane's wrapper fix: the existing Nix-store OpenSSL is used for compile/link,
and LD_LIBRARY_PATH is set for test binaries. No test binary failed to load
libssl. This lane did not edit the wrapper.

Commands, run from the workspace root:

```sh
./scripts/with-toolchain.sh cargo check -j4 -p calternal-auth
./scripts/with-toolchain.sh cargo fmt -p calternal-auth
./scripts/with-toolchain.sh cargo test -p calternal-auth -j4
./scripts/with-toolchain.sh cargo test --workspace -j4
./scripts/check-docs.sh
RUSTDOCFLAGS='-D warnings' ./scripts/with-toolchain.sh cargo doc --locked -j4 -p calternal-auth --no-deps --all-features
```

- OpenSSL/WebAuthn prerequisite check: exit 0 after the wrapper fix.
- Format: exit 0, only this crate selected.
- Auth: exit 0; **77 passed, 0 failed, 0 ignored** (37 inline, 38 account
  integration, 2 migration integration). Doc tests: 0. This includes 43 source
  cases in the step-4 ledger, of which 2 were formerly ignored migrations;
  11 new step-4 regressions; and 23 existing runnable step-3 cases.
- Workspace: exit 0; **197 passed, 0 failed, 0 ignored**, doc tests 0. Server
  emitted 47 pre-existing missing-doc warnings; its files are outside this lane.
  This result describes the then-current shared working tree, including other
  lanes' changes, and does not claim production qualification.
- Documentation gate: exit 0, Rust check/doc and TypeScript documentation checks
  passed. The gate now includes auth/fsio/roots. It was repeated after final
  comment edits, with no product-code changes after the test runs.
- Strict auth rustdoc: exit 0, no warnings or broken links.

Source SQL check: all fifteen SCHEMA_V* migration string values match pinned
`ea57b84b` byte for byte. Version remains 15, and source-role/capability tables
and sharing tables remain in place. Only now_ms/sql helpers were restored to
db.rs for the newly ported account layers; migrations were not changed.

The real-passkey migration tests retain credential/session bytes and test
successful post-migration login, recovery-token check and pending enrollment.
The people.list transport assertion is expressed against the same persisted
role/quota rows; no people API was added. Original test bodies remain in
`tests/fixtures/source/` as evidence.

HTTP/cookies, account event publication, OIDC, invites, groups, quota, sharing,
app passwords, DAV and FO1 security-before-ack coordination remain outside this
bounded library slice. Root validation and privileged administrative identity
verification are the host caller's obligations; see the public API docs and
PORTING.md for the adaptations and the process-local session-cache limitation.

## Questions for the user

None for this slice.
