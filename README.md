# Calternal-AI

A file-first AI orchestrator and personal productivity app, under development. The architecture uses a Rust host and shared contracts/client core, TypeScript plugins, portable GenUI, and Swift native clients through UniFFI. The browser uses TypeScript, without Wasm.

## Local development preview

This branch implements a **disposable-data engineering preview**, not the complete orchestrator or a daily-use replacement for Calternal. It includes real Notes operations, version history, permission checks, a trusted TypeScript statistics extension, web GenUI and the `cl` CLI. The AI pane clearly indicates that harness setup is not implemented.

Requirements: Linux/macOS build environment, pinned Rust 1.99.1, a C linker, Node 22.13 or later, npm and Python 3 for the CLI integration check. The current server has been exercised on Linux; macOS and Apple bindings need their own execution checks. `scripts/with-toolchain.sh` uses an installed C compiler and limits Cargo concurrency; it does not install system packages.

From the repository root:

```sh
npm ci
npm run build
./scripts/with-toolchain.sh cargo build --locked -p calternal-server -p calternal-cli
./scripts/dev-preview.sh
```

Open `http://127.0.0.1:3210`. The server prints **paths**, never token values. Privately open `.dev-data/dev-tokens/owner.token` and enter its contents in the connection screen. The browser holds it in memory only, so reloading requires reconnecting. Do not paste tokens into a conversation or include them in screenshots.

The owner can edit `personal` and `shared`; `.dev-data/dev-tokens/guest.token` can only read `shared`. These are fixed development principals, not production authentication. The server binds to loopback only. When running on a remote development machine, use an existing trusted port-forward/browser session; do not expose the preview publicly.

Try creating and editing a note, inspect its history, enable Notes insights under Plugins, and open its statistics panel. A stale save preserves the draft and requires explicit resolution. Stable note links use `/s/<space>/notes/<id>`.

The CLI uses the same operation route:

```sh
export CALTERNAL_TOKEN_FILE="$PWD/.dev-data/dev-tokens/owner.token"
./target/debug/cl describe
./target/debug/cl notes list --space personal
./target/debug/cl plugins list --space personal
```

For mutation retries, retain the `--request-id` and repeat exactly the same operation and input. A transport failure can mean the server committed even though the reply was lost. Tokens are accepted from a protected file or environment, never a command-line token argument.

## Verification

`./scripts/verify-preview.sh` builds the packages, checks generated-contract drift, runs web/SDK/Rust tests and backend integration checks. Browser interaction verification is documented separately in [the UI report](docs/ui-preview.md); automated unit tests alone are not a browser proof.

`python3 contract/verify-cli.py --running-data-dir .dev-data` checks the CLI against a running preview using disposable fixtures. `node scripts/measure-preview.mjs` launches a separate disposable server and records local debug timings in `artifacts/preview-smoke.json`. Those timings are not a 120 Hz UI result or a production performance gate.

## Scope and limitations

- SQLite is the preview's authoritative revision/receipt store. Markdown files are checked materialized copies. **Do not import existing Calternal data:** inherited file-only recovery, tiering, external-edit import and production migrations are not proven here. Detected external edits block writes; concurrent external writers are unsupported.
- Only repository-shipped, digest-pinned plugin bundles run. Node permission flags and cleared environments are defense in depth, **not an OS sandbox**. No arbitrary community plugin installation or provider credentials.
- Notes uses a textarea, without collaborative CRDT editing or offline support. Activity is limited to the current browser session. There is no real AI conversation, machine pairing, provider login, voice or memory integration yet.
- UniFFI exposes generated contract/validation scaffolding. This is not a finished native networking/sync core or a working Apple app.
- Two low-severity npm audit findings affect a build-tool dependency; the UI report records their review. No production-readiness claim is made.

The full product plan remains separate from this deliberately bounded implementation:

- [Architecture](docs/architecture-proposal.md)
- [Early-use plan and review reconciliation](docs/early-use-plan.md)
- [Overnight contract](docs/tonight-contract.md)
- [Inherited decisions and storage audit](docs/inheritance-audit.md)
- [Validation plan](docs/validation-plan.md)
- [Morning decisions](docs/morning-decisions.md)

UI design and implementation are Claude-only. See [AGENTS.md](AGENTS.md).

## License

AGPL-3.0-only; see [LICENSE](LICENSE). Dependencies retain their respective licenses.
