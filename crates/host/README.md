# Calternal host

`calternal-host` serves personal spaces through one session-authenticated
call route. It keeps items in Markdown and stores the authoritative item
heads, record authors, private viewer state and retry receipts in a
per-space SQLite ledger. The query index can be rebuilt from the ledger
and the Markdown.

Run `calternal-host serve ROOT ORIGIN ADDRESS` with a dedicated root.
`calternal-host install ROOT ORIGIN OWNER SPACE DIRECTORY [--official]`
installs a bundle from the local operator's terminal when the serving
process has released the space. The owner must already have an account
and a personal space in the root's auth directory. Set
`CALTERNAL_RECEIPT_RETENTION_DAYS` to a positive whole number to override
the seven-day default.

Serve the web bundle and proxy `/v1` (including WebSocket upgrades) from
the configured public HTTPS origin. The browser uses `GET /v1/auth/status`,
the passkey ceremonies under `/v1/auth/login` and `/v1/auth/setup`, and
`POST /v1/auth/logout`. The auth service issues a Strict, Secure, HttpOnly
host cookie. It is also required by `GET /v1/spaces/{space}/live`.
First-owner enrollment requires an operator-issued setup code. If the proxy
sets `X-Forwarded-For`, set `CALTERNAL_TRUSTED_PROXIES` to the actual proxy
networks in CIDR notation so passkey rate limits use the transport client.

The [call contract](../../docs/reviews/host-call-route-2026-11-27.md)
describes worker and browser messages. The
[verification and findings](../../docs/reviews/host-core-findings-2026-11-27.md)
track format-engine integration and append recovery.

## Declaration-only apps

Outside-editable kinds have built-in add, change and remove actions. An
item-field assignment uses `s.write` with an entry such as
`{"op":"perform","item":"<uuid>","operation":"<name>","fields":{}}`.
Send only values declared as `ask`, and include `base` when changing a
field in a declared group. The host evaluates the operation condition
against the current row and publishes the patch as one item version.

A kind or collection query accepts optional `search` text. It matches
declared stored and computed fields and the Markdown body when declared.
Grouped collections return per-row group keys and group totals over all
matched rows, before pagination. The catalogue supplies field types,
collection views, operations and declared presentation metadata. Formula
reads use the current UTC date. Resident spaces invalidate live projections
on the first maintenance sweep after the UTC date changes.

## Coded plugins on the personal-space host

The operator can enable the isolated worker runtime for both `install` and
`serve` by setting all three paths:

- `CALTERNAL_WORKER_BWRAP` is the absolute Bubblewrap executable.
- `CALTERNAL_WORKER_NODE` is the absolute Nix store Node 24 executable.
- `CALTERNAL_WORKER_SDK` is the absolute SDK package directory with its built
  `dist/` and `package.json`.

The install probe reads exported selectors from the sandbox. The host
checks those selectors against the installed declarations and saves them
with the verified bundle. The runtime starts a worker for the space and
plugin on demand and kills it after idling. Only the read-only runtime,
SDK and code snapshot are mounted for the worker. The host route checks
current space and plugin authority on each callback. An existing data-only
installation has no probed registrations and coded calls stay disabled
for it.

The ordinary workspace gate builds the SDK before running the Linux
sandbox tests. For a focused dev VM run:

```sh
~/bin/lock-shared nix develop -c bash -c '
  set -e
  npm run build --workspace @calternal/plugin-sdk
  cargo test --locked -p calternal-host --test coded
'
```

Bubblewrap and Node come from the pinned dev shell. Set
`CALTERNAL_TEST_BWRAP` or `CALTERNAL_TEST_NODE` to override their paths.

## Harness runs on Linux

Set `CALTERNAL_RUN_IMAGE` to a local provider image to enable the Podman
runner. The host locks its runs directory and clears leftover containers
for that root when serving starts. Install Chat before installing the
harness. A fresh developer VM checkout can use the
[fake provider image](../../plugins-v2/calternal-chat-claude-code/README.md).

Start, input and Stop use the [host call route](../../docs/reviews/host-call-route-2026-11-27.md).
Run output uses the [live window socket](../../docs/reviews/live-window-protocol-2026-11-27.md).
Input receipts report `queued` when the adapter channel accepts them. Stop
fences host callbacks before terminating the process family. Removing the
target also fences and terminates its runs.
After a host restart, a repeated start request returns the original run ID
with status `stopped`. The live log starts empty with the new host process.

## Disposable Chat preview

On the dev VM, run `scripts/dev-chat-host.sh` from this checkout. The script
builds the web client, standalone host and fake provider under the shared
build lock. It serves the client on a free loopback port and prints its URL
and a private first-owner code file path. Open the URL locally or forward
that same port over SSH. Complete passkey enrollment with the code from the
file. The host installs Chat, its harness, Reading List and Tasks into
the owner's space at sign-in or on an authenticated session-status check.
Existing bundles are left in place. Stop the preview with Ctrl+C.

The [persistent development preview](../../docs/dev-chat-preview.md) uses a
loopback user service and immutable release image IDs. A temporary failure to
install a bundle leaves the committed sign-in result and first enrollment's
recovery codes available. An authenticated status check or later sign-in
retries missing bundles. The fake image is the default. Real sign-in requires
explicit account, space, program, image ID and credential-file settings.
