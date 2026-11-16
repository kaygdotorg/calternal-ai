# Core/client/CLI lane handoff — 2026-11-16

Completed without UI changes, descendants or commits. Root `Cargo.toml` uses
`crates/*`; crate names are `calternal-core`, `calternal-client`, `calternal-cli`
(binary `cl`) and `calternal-bindings`. Rust 1.99.1, two build jobs; use the
parent-owned `scripts/with-toolchain.sh` compiler wrapper. Cargo.lock was generated
by normal workspace builds; parent owns final integration/commit.

## Published artifacts

- `crates/core/src/models.rs`: strict wire/resource/GenUI/plugin/worker models.
- `crates/core/src/validation.rs`: pure synchronous validation, recursively
  canonical request input, optimistic revision check and retained pending identity.
- `contract/v0.schema.json`, `contract/operations.json` and
  `packages/contracts/src/index.ts`: generated from Rust, with read-only
  `export-contract --check` drift gate. Shapes were announced to all lanes first.
- `contract/fixtures`: valid/hostile shared conformance corpus, including optional
  null rejection and required nullable response fields.
- `crates/client`: typed and generic reqwest adapter, matching response identity,
  typed conflicts, sensitive bearer header, loopback URL restrictions, no proxy
  or redirects. Mutating convenience helpers require caller-retained request IDs.
- `crates/cli`: clap Notes/plugin/discovery/generic invocation with JSON results,
  token file/environment only, Markdown file/stdin and stable request identities.
- `crates/bindings/generated/swift`: generated synchronous validation facade,
  header and module map. No native renderer/network client or Apple execution proof.
- Usage/boundaries: `contract/README.md`, `crates/bindings/README.md`.

The parent explicitly authorized the sole shared-contract clarification:
pre-dispatch authentication/Host/Origin errors may have request_id:null. Clients
retain their own pending identity and reject mismatching nonnull IDs; successful
POST responses must echo it. Successful GET describe is the bare DescribeResult.

## Verified

Passed:

```sh
scripts/with-toolchain.sh cargo test -p calternal-core -p calternal-client -p calternal-cli -p calternal-bindings --features calternal-bindings/uniffi
scripts/with-toolchain.sh cargo run -p calternal-core --bin export-contract -- --check
scripts/with-toolchain.sh cargo clippy -p calternal-core -p calternal-client -p calternal-cli -p calternal-bindings --all-targets --features calternal-bindings/uniffi -- -D warnings
scripts/with-toolchain.sh cargo fmt -p calternal-core -p calternal-client -p calternal-cli -p calternal-bindings -- --check
scripts/with-toolchain.sh cargo build -p calternal-cli -p calternal-bindings --features calternal-bindings/uniffi
target/debug/calternal-bindgen generate --library target/debug/libcalternal_bindings.so --language swift --out-dir crates/bindings/generated/swift
python3 contract/verify-cli.py --running-data-dir .dev-data
python3 contract/verify-cli.py
```

23 Rust tests pass: ten core conformance tests, seven HTTP tests, four CLI tests
and two bindings tests. The HTTP corpus includes a fully received mutation whose
response is dropped, followed by a retry asserting identical request ID and JSON
bytes. It also covers typed conflicts, discovery shape, mismatched IDs, null
pre-dispatch error IDs and URL/header credential rejection. Core checks fixture
parity, schema/catalog drift, worker framing/version/ID, semantic tree limits and
an extension falsely declaring a mutating host action as read-only.

The real CLI test passed against the parent's running :3210 server. After fixing
fixture IDs to use a UUID per run, it passed twice successively against that same
data directory. The isolated test also passed after the final CLI build. It
spawns its own disposable server, exercises describe/create/get/list/update/history,
stale conflicts, exact retries, guest personal-read/shared-write/plugin-management
denial and guest shared-read access, then restarts and verifies retained content,
revision/history and retry receipts. Temporary server/data cleanup is confined to
its own temporary directory. No token values were printed or placed in arguments.

## Independent server interface review

Read-only review found note/plugin mutation receipts now use the same
principal+request-ID namespace, compare operation plus canonical fingerprint,
check current roles before replay, and share store serialization. Reads scope
both resource ID and space. Worker results are checked for exact protocol and
invocation identity, schema/tree bounds and declared read-only actions, then
plugin generation is rechecked before delivery.

One actionable recovery gap was sent to Opus/parent: when pending replay sees the
intended file bytes already present, it must still sync the file and directory
before marking placement durable. The server lane added those syncs; source
correction was observed in `Store::materialize`. Backend fault-test acceptance
remains the server/parent lane's verification responsibility.

## Explicit limits

This is disposable preview evidence only. Core has no HTTP, async or UniFFI;
client owns HTTP; bindings only validate synchronous JSON. Swift generation and
Rust feature compilation passed on Linux; Swift/Apple compilation and execution
were not tested, and swift-format was unavailable. Dropping a client future does
not cancel a server commit. No persistent local outbox, offline/reconnect,
production authentication, hostile-plugin confinement, inherited file-only
recovery/tiering or real-data migration is claimed. HTTP responses have a 128 MiB
cap; large history pages can require a smaller requested limit. No performance
or native UI claim is made.
