---
title: "Turn readiness"
---

<!-- Generated from: docs/agent-turns.md. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

`agent.readiness` takes `space_id` and an optional saved `thread_id`. It
requires a person with `plugins.use` in the space. Unknown and inaccessible
spaces or threads return the same not-found response. A member cannot read
another member's personal machines, even when both can read the same thread.
The caller's active, same-machine pin from a person request limits the
thread-scoped snapshot.

The result contains `machines`, `pool` and `check_ttl_ms`. A personal row
has the enrolled machine UUID and name, `enrolled`, `connected`,
`last_seen_ms` (null when this server has seen no connection), `execution`
(`direct` or `sandboxed`), `available` (placement capacity), and
`providers` keyed by provider ID. An entitled account receives anonymous pool
options with the same capacity and provider fields. Pool rows omit host
identifiers, names and timestamps. Without pool entitlement the array is empty.
Each provider has `enabled` for the space, `check` (null until observed or
after expiry), and `ready` when enabled, available and signed in. An observed
check contains only `installed`, `signed_in` and `sign_in_command`.
The provider's official program owns the login.

Use a cached read with `calternal agent readiness --space-id SPACE_UUID`.
Refresh a provider with `calternal agent check --space-id SPACE_UUID
--machine-id MACHINE_UUID --provider PROVIDER_ID`. `--pool` checks an
available entitled pool machine. A Check job is private to its admitting
account. The scratch workspace is released after the check and the sandbox
home is retained. The Claude adapter reports the official program's status
during regular turns as well.

The cache is in memory, keyed by account and provider on the selected
machine. It holds at most 128 observations per machine for five minutes each.
The oldest observation is evicted at capacity. Expiry or eviction returns
the state to unknown. Restart clears the cache. Changes made outside Calternal
become visible after the next provider check.

Subscribe with `GET /api/v1/account/live?readiness=true`. The private
`{"type":"readiness_changed"}` frame asks the account to reload readiness
when its machines connect or drop, capacity changes, or its provider
observations change or expire. This coalesced frame has no bookmark or machine
payload. Read a snapshot after reconnect. Pool machine notifications go only
to current pool-granted subscribers, and provider observation notifications go
only to the account checked.

The warm read takes the existing space capability lookup, a single indexed
space-local SQL statement for activations and any pin, and one owner-indexed
in-memory registry snapshot. No body reads, flushes or machine traffic. A cold
space opens its existing indexes. A check uses the ordinary fenced lease and
job receipt. Cache publication and invalidation add no durable writes.
