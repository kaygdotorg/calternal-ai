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
