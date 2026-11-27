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

The [call contract](../../docs/reviews/host-call-route-2026-11-27.md)
describes worker and browser messages. The
[verification and findings](../../docs/reviews/host-core-findings-2026-11-27.md)
track format-engine integration and append recovery.
