# Calternal-AI

A file-first AI orchestrator and personal productivity app, under development. The architecture uses a Rust host and shared contracts/client core, TypeScript plugins, portable GenUI, and Swift native clients through UniFFI. The browser uses TypeScript, without Wasm.

## Local development preview

This branch implements a **disposable-data engineering preview**, not the complete orchestrator or a daily-use replacement for Calternal. It includes real Notes operations, version history, permission checks, a trusted TypeScript statistics extension and the `cl` CLI. Saved conversations now use manifest-owned thread operations,
Markdown files and `calternal threads`. Checkpoint journal entries support
incremental reads for clients. See the [thread format and limits](docs/threads.md).
There is no web UI yet; it will be built from [docs/ui-shell.md](docs/ui-shell.md).

Requirements: Linux/macOS build environment, pinned Rust 1.99.1, a C linker, Node 22.13 or later, npm and Python 3 for the CLI integration check. The current server has been exercised on Linux; macOS and Apple bindings need their own execution checks. `scripts/with-toolchain.sh` uses an installed C compiler and limits Cargo concurrency; it does not install system packages.

From the repository root:

```sh
npm ci
npm run build
./scripts/with-toolchain.sh cargo build --locked -p calternal-server -p calternal-cli
export CALTERNAL_INSTALLATION_ID=$(cat /proc/sys/kernel/random/uuid)  # keep it; see below
./scripts/with-toolchain.sh cargo run --locked -p calternal-server -- serve \
  --development --create --port 3210 --plugin-root plugins \
  --appdata .dev-data/appdata \
  --userdata-ssd .dev-data/userdata-ssd \
  --userdata-hdd .dev-data/userdata-hdd
```

The server keeps its databases in appdata and space files in the two userdata
roots. The installation ID is pinned in the server's configuration
(`CALTERNAL_INSTALLATION_ID`, or `--installation-id`), outside every root: keep the
value you generated with the rest of the server's configuration. `--create` pairs
three empty roots with it on first start; afterwards the server refuses to start
without the pin, or with roots that are missing, swapped, or carry another ID. Security
changes are also written to a protected log in `userdata-ssd/.calternal/recovery`.

`--development` creates two fixture accounts and writes their bearer tokens to
`.dev-data/appdata/dev-tokens/`. The server prints **paths**, never token values.
Do not paste tokens into a conversation. The owner has two spaces named
`personal` and `shared`; the guest can only read `shared`. These are fixed
development principals, not production authentication. The server binds to
loopback only; do not expose the preview publicly.

The CLI uses the same operation route. Spaces are named by UUID, listed by
`describe`:

```sh
export CALTERNAL_TOKEN_FILE="$PWD/.dev-data/appdata/dev-tokens/owner.token"
./target/debug/cl describe
./target/debug/cl notes list --space <space-uuid>
./target/debug/cl plugins list --space <space-uuid>
```

Notes, Tags and Trash list and lookup commands return up to 100 rows per page.
Reuse the same arguments with `--cursor` to continue.
See [Notes operations](docs/notes-operations.md).

Use `calternal search` to find text across readable spaces and browse picker
and palette data. See [Search and palette lookups](docs/search.md).

Use `calternal people`, `calternal groups`, and `calternal invites` to
manage space access through the same HTTP transport. See [people and access](docs/people.md).

For mutation retries, retain the `--request-id` and repeat exactly the same operation and input. A transport failure can mean the server committed even though the reply was lost. Tokens are accepted from a protected file or environment, never a command-line token argument.

Space changes can be followed with `calternal watch <space-uuid> --token-file
<credential-file>` (`--json` emits one event per line). The shared client follows
one SSE connection, retains accepted bookmarks, invalidates bounded item caches,
and drains ordered offline input after reconnect. Swift exposes a cancellable
async stream. [Private drafts, pending submissions and the account feed](docs/account-input.md)
use per-account storage, explicit-delete retention and seven-day confirmation
for old offline input. See the [drafts lane report](docs/reviews/drafts-2026-11-17.md)
for qualification and remaining platform work.

## Verification

`./scripts/verify-preview.sh` builds the packages, checks generated-contract drift, runs SDK/Rust tests and backend integration checks. Where Podman is installed, it checks for containers left by the workspace tests.

### UI merge gate (browser suites)

For every merge that touches `apps/web`, `packages/ui`, `packages/editor` or `packages/genui`, from a clean checkout:

```sh
nix develop -c sh -c 'npm run build -w apps/web && npm run e2e:gate -w apps/web'
```

`e2e:gate` serves `apps/web/dist` once on a free port and runs the deterministic headless suites on fixtures that guard decided behaviour (first visit, shell audit, flat sidebars, resize, sidebar scroll, accessibility, text parity, palette and its focus, interaction, phone navigation, Settings pages, focus rings, swipe rows, morph, states, new thread), six at a time (the accessibility walk is four parallel parts), each under its own timeout. It prints one line per suite and exits non-zero on any failure (a failing suite also prints its last lines). `node e2e/gate.mjs <suite> ...` runs some. The suites that need a real server build are `npm run e2e:server -w apps/web` (builds the server binary and the app, then sign-in, setup accessibility, people, machines and start-up against a disposable installation); run it when a merge touches server mode. Chromium comes from the nix shell (`flake.nix` sets `PLAYWRIGHT_BROWSERS_PATH`, `FONTCONFIG_FILE`; `playwright` in `apps/web/package.json` is pinned to the version of nixpkgs' `playwright-driver`, so raise both together), so nothing is installed per machine. The suites need the default wallpapers, which `npm run build` fetches from the image host; a machine the host refuses (dev-01) takes them from `~/.cache/calternal-ai/wallpapers` (copy the folder from a machine that fetched them, e.g. dev-02), and the gate says so before it starts if they are missing. Suites left out of the gate, and why, are in `apps/web/e2e/README.md`.

`python3 contract/verify-cli.py --running-data-dir .dev-data` checks the CLI against a running preview using disposable fixtures. `node scripts/measure-preview.mjs` launches a separate disposable server and records local debug timings in `artifacts/preview-smoke.json`. Those timings are not a 120 Hz UI result or a production performance gate.

## Reference documentation

Every public code change includes matching reference comments. Run `npm run check:docs` to enforce documentation coverage and links, or `npm run build:docs` to generate the Rust and TypeScript sites under ignored `reference-docs/`. See [documentation rules and commands](docs/documentation.md) for entry pages, tool versions and server-lane integration.

## Scope and limitations

- Each space's SQLite databases in appdata are authoritative for its revisions and request receipts; Markdown files in userdata are checked materialized copies. **Do not import existing Calternal data:** inherited file-only recovery, tiering, external-edit import and production migrations are not proven here. Detected external edits block writes; concurrent external writers are unsupported.
- Only repository-shipped, digest-pinned plugin bundles run. Workers use the machine role’s container sandbox; Node permission flags and cleared environments add defense in depth. Explicit direct development mode has no OS isolation. No arbitrary community plugin installation or provider credentials.
- Notes uses a textarea, without collaborative CRDT editing or offline support. Activity is limited to the current browser session. Saved thread files and manual provider turns are development slices, with steering,
large-output storage and the public TypeScript product bridge still unfinished.
The private [request inbox](docs/requests.md) lists agent questions, approvals and
secret prompts through `calternal requests` and the account feed.
The saved Threads preview includes a provider model catalog, live work phases,
and per-message history. See [manual turns](docs/agent-turns.md) and
[saved threads](docs/threads.md). Voice and
memory integration remain unfinished.
- UniFFI exposes async operations and cancellable journal following; subscribe/cancel is exercised by XCTest on the authorized Mac Studio. Platform persistence, Keychain integration and the native app remain unfinished. The preview HTTP client accepts loopback origins only.
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

Plugin workers execute through the machine role’s existing sandbox interface.
See [worker execution](docs/plugin-workers.md) for the container profile, runtime
setup, lifecycle bounds and unresolved placement/network contracts.

Notes supports title and tag pickers, relative-link rename, and 30-day space
Trash. See the [Notes operations reference](docs/notes-operations.md).

The [deployment runbook](docs/deploy.md) covers the rootless Podman image, machine service and calternal.cloud cutover.
