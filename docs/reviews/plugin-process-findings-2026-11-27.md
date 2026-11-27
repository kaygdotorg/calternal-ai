# Coded operation process findings · 2026-11-27

Lane C, slice 3. [Forgejo #116](https://git.kayg.org/kayg/calternal-ai/issues/116) ·
[Frame contract](plugin-frames-2026-11-27.md) ·
[Build plan](../proposals/chat-first-build-plan-2026-11-27.md) ·
[Format revision 9](../proposals/kind-and-collection-format-2026-11-24.md).

## Gate and slot follow-ups

- Linux's pinned Nix dev shell supplies Bubblewrap. The ordinary workspace
  gate builds the SDK before running the sandbox tests. They look up
  Bubblewrap and Node on PATH and keep the existing optional overrides.
- Chat no longer lists the Claude Code harness in `plugin.md`. A person's
  `slot_selection` names the declared slot and installed filler. The host
  resolves its canonical coded operation and scopes permission to the
  initiating worker. The selected operation is visible in its event for
  dispatch. Nested calls and change handlers receive no inherited slot
  grant. The route exercises the selection, replay, an installed
  non-filler, a different operation and an input-only harness hint.
- The fixture removes the `calls:` declaration at former
  `plugin.md` line 7. In `operations.ts`, line 16 documents the optional
  operation hint, line 37 resolves the selected operation, line 38 tests
  its presence, and line 43 calls it with the canonical record link.

## Real host integration

The asynchronous session and trusted adapters now dispatch declared code
and route its `s.query`, `s.read`, `s.write`, and `s.state` callbacks
through lane B's host. `s.me` and `s.space` use the verified personal-space
call context. The install probe runs against a private snapshot of the
verified bundle in a Bubblewrap user, network and PID namespace. The
worker has a read-only SDK and source, a read-only Nix store and no home or
host data mounts. The Node permissions also restrict filesystem access.
The host stores the selector catalogue beside the bundle digest. It runs
registered observers after commit and outside the space lock. Nested
operations use declared calls and suppress observations back into their
own call chain.

An append returns lane B's canonical record link. Chat uses it in the
selected harness call under the same host-owned request lineage. The
current host shell has no scope-aware run adapter, so a requested
unavailable harness returns `not-allowed`. The real-host test retries
that gesture and verifies there is still one append. A separate nested
rename exercises the authorized `s.call` route against installed Chat.
Real run startup and deduplication depend on the runs lane.

### Fixture edits in the original integration

| File | Changed lines in this branch |
| --- | --- |
| `plugins-v2/calternal-chat/kinds/chats.md` | 13 counts after the stored record anchor. 19 changes the read cursor to a record anchor. |
| `plugins-v2/calternal-chat/plugin.md` | 7 declares the Claude Code harness's qualified start operation. |
| `plugins-v2/calternal-chat/operations.ts` | 16 documents the qualified start operation. 17 adds it to the send input. 26 binds it in the send handler. 31 awaits the append receipt. 37 checks for a harness. 38 reads the record link. 39–41 return a safe error if the link is missing. 42 passes the chat and prompt link to the harness. 43 closes the conditional. 44 returns the append receipt. |

### Integration boundaries

- The personal-space route verifies its one owner before supplying an
  actor and on every effect. A wider shared-space launcher needs a
  decision on cross-actor process reuse. The operator opt-in requires
  Bubblewrap and the host has no direct Node fallback.
- Change delivery currently occurs after the committed route and
  coalesces rows inside one write. The registration survives a restart.
  An interruption after a commit can lose a notification because this
  slice does not add a durable delivery queue. A wider at-least-once
  delivery design needs to preserve the fast append's two flushes.
- The route admits a selected harness through the person's verified slot
  selection. Coded conditions are refused until the host can check their current
  viewer context. Cross-plugin calls check recognized item designations
  and reduce unreadable structured rows to IDs. Opaque extension outputs
  still require a host policy if they need stronger data-flow controls.

## Transport implementation choices



| Section | What the lane chose and why |
| --- | --- |
| §8.1 and lane B’s route | The SDK uses the revision 9 fixtures' one-key change entries and translates their operation names to the route's `op` tag. A record goes through as `{fields?,body?}` so the Markdown stays in the body. `s.me`, `s.space` and `s.event` use the trusted invocation metadata locally. Session is the only run scope exposed by this slice and the SDK supplies the default. |
| §8.1 errors | `calternal_types::Error` uses older code spellings and a `details` field. The wire has the ten revision 9 codes and `next`. A pipe-specific code conversion retains the shared Rust error value and serializes the format reply only at the boundary. |
| §8.2 events | The registry provides admission for declared calls and registered change selectors. The SDK accepts one exported handler per event kind and selector, so every replay picks the same handler. A busy endpoint returns `limit`, allowing the trusted scheduler to retry outside a call chain. This also avoids workers waiting on each other. The isolated install probe caches selector names since the Markdown manifests do not enumerate handler exports. The active route supplies the acting person and call ancestry. A durable queue remains outside this integration. |
| §8.2 replay and eleventh set | The transport derives stable receipt keys from the event key and the ordinal within each effect type. Nested calls preserve the original gesture context and get their own stable event keys. The harness service must reuse the nested `s.run` receipt. A killed worker before commit leaves no write in the mock. A kill after commit can withhold the reply, and the stable key lets a retry obtain the earlier receipt. |
| §8.1 and §7.4, eleventh set | The send-and-start gesture uses the host-issued `link` on the append receipt. An anchor is not a path-correct Markdown link by itself. The transport carries the typed payload through unchanged and leaves validation to the shared route. |
| §2.9, §4.3 and §7.4, eleventh set | Append receipts expose anchors and state accepts an authorized JSON value. Ledger-order counting and the shared typed input declaration belong to the format and call route lanes. The generic SDK handler signature can describe the input for author tooling and leaves runtime validation to that declaration. |
| §7.1, §8.1, eleventh set | A run callback sends the program ID and scope data. The installed binding from that ID to the contained executable or image stays with lanes B and D. Worker inputs carry no program paths. |
| Build plan slice 3, existing workspace graphs | `crates/server` imports the old registry and runtime, `crates/machine` imports its space channel, and current npm plugins import the original SDK. Removing those APIs now would break workspace consumers outside this lane. The Rust `operations` feature and Node `calternal-operations` condition select the new transport and SDK. No adapter translates the old protocol into the new one. |

## Security questions for the coordinator

- **Worker containment and reuse (§8.2, primitives-only host T1–T3).**
  The process supervisor clears its inherited environment and owns the
  pipes. The personal-space launcher uses Bubblewrap with a read-only Nix store,
  SDK and source snapshot, and Node's permissions and heap bound.
  For a shared space, what runner resource and actor-reuse limits should
  the operator enforce? A plugin retaining one person's result in module state
  could expose it in a later person's call. Reusing processes across
  people saves cold starts, while recycling on actor change reduces
  retained state and still permits one live worker per installation.
  Keep cross-actor reuse disabled pending that decision.
- **Pipe budget and admission (§8.1–8.2).** An allowed 32 MiB read can
  exceed 64 MiB when its text needs escaping. The current frame limit
  refuses the oversized result and keeps buffering bounded. Raising it
  increases worst-case memory per active worker. Chunking adds a framing
  state and preserves large reads. Decide the production frame and
  resource budget with the sandbox and route owners. Keep the bounded
  mock behavior for the slice.

## Review gate and counts

- Reused the existing bounded JSON serializer and strict JSON parser,
  the SDK’s newline transport, shared account and space IDs, and the
  shared caller-safe error. The spike and earlier v2 lanes informed reuse
  review. The new host route and future run service are the callers for
  the public transport and SDK.
- One owner task per installed coded plugin, no global request lock and
  no SQLite dependency. The registry owns declaration lookup. The SDK
  builds its handler map once per worker. Process spawn runs off the
  async executor, and pipe reads and writes are asynchronous.
- Warm Chat send performs **zero storage reads and zero durability
  flushes in slice 3**. It uses four JSON frames, two in each direction,
  and parses each incoming frame once. Send-and-start in the mock uses
  ten frames across two workers. Filesystem work and the append flush
  budget are owned by lane B. No timing or flush benchmark is claimed
  from the development VM.
- The lane keeps no persistent data. The route owns the receipt retention
  and input-conflict behavior. Malformed or stale pipe data kills the
  current worker. Known operation failures can leave a healthy worker
  available.

## Verification

The focused gate ran in `~/Developer/lanes/plugin-process-slice3` on
`claude@calternal-dev-01-compute02` with `~/bin/lock-shared nix develop -c`.
Formatting passed. The three Rust packages passed 87 tests, including four
Bubblewrap tests hitting installed Chat on the real host. The SDK passed
24 tests, and the two revision 9 operation modules type-checked. Lower-level
wire tests retain the mock run service until lane D attaches the real one.

Tracing a steady-state coded append on that VM showed 0 item reads,
0 ledger reads, 0 auth reads, 1 item flush and 1 ledger flush. After
a rename invalidated the cached file signature, the first append read
the item twice and still performed two flushes. Both traces scope the
counts to the real operation and report read syscalls, not logical
authorization checks or performance timings.

The workspace gate ran under `~/bin/lock-full nix develop -c`. It checked
all workspace targets and features and the standalone conformance
manifest, built the bundled plugins and generated Rust and TypeScript
references. The corresponding generated public references are included in this
integration branch. Rust and SDK tests ran on the same dev VM.

## Simplification opportunities

- Move the ten caller-safe codes and `next` into shared types once the
  new call route is the workspace default.
- Remove the v1 registry, worker entry and export condition together
  when the server, machine and bundled preview plugins migrate.
- Derive slot, operation and event registration metadata from the shared
  install declarations and drop the transport's need to learn it.
