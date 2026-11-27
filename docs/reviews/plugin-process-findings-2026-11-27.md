# Coded operation process findings · 2026-11-27

Lane C, slice 3. [Forgejo #116](https://git.kayg.org/kayg/calternal-ai/issues/116) ·
[Frame contract](plugin-frames-2026-11-27.md) ·
[Build plan](../proposals/chat-first-build-plan-2026-11-27.md) ·
[Format revision 9](../proposals/kind-and-collection-format-2026-11-24.md).

## Implementation choices

| Section | What the lane chose and why |
| --- | --- |
| §8.1 and lane B’s route | The SDK uses the revision 9 fixtures' one-key change entries and translates their operation names to the route's `op` tag. A record goes through as `{fields?,body?}` so the Markdown stays in the body. `s.me`, `s.space` and `s.event` use the trusted invocation metadata locally. Session is the only run scope exposed by this slice and the SDK supplies the default. |
| §8.1 errors | `calternal_types::Error` uses older code spellings and a `details` field. The wire has the ten revision 9 codes and `next`. A pipe-specific code conversion retains the shared Rust error value and serializes the format reply only at the boundary. |
| §8.2 events | The registry provides admission for declared calls and registered change selectors. The SDK accepts one exported handler per event kind and selector, so every replay picks the same handler. A busy endpoint returns `limit`, allowing the trusted scheduler to retry outside a call chain. This also avoids workers waiting on each other. Lane B needs registration metadata from the install path for `on.change` because the Markdown manifests do not enumerate handler exports. A launcher can discover registrations once in its isolated install probe and store only selector names. Actor rechecks, candidate filtering, coalescing, ancestry, reaction provenance, self-suppression and at-least-once delivery belong to lane B. |
| §8.2 replay and eleventh set | The transport derives stable receipt keys from the event key and the ordinal within each effect type. Nested calls preserve the original gesture context and get their own stable event keys. The harness service must reuse the nested `s.run` receipt. A killed worker before commit leaves no write in the mock. A kill after commit can withhold the reply, and the stable key lets a retry obtain the earlier receipt. |
| §8.1 and §7.4, eleventh set | The send-and-start mock requires the exact appended record link. `anchor` alone from lane B’s draft cannot form a path-correct Markdown link. The SDK accepts a host-issued `link` on the write receipt. Lane B needs to confirm and supply it or agree on another canonical link representation. The transport carries the typed payload through unchanged and leaves validation to the shared route. |
| §2.9, §4.3 and §7.4, eleventh set | Append receipts expose anchors and state accepts an authorized JSON value. Ledger-order counting and the shared typed input declaration belong to the format and call route lanes. The generic SDK handler signature can describe the input for author tooling and leaves runtime validation to that declaration. |
| §7.1, §8.1, eleventh set | A run callback sends the program ID and scope data. The installed binding from that ID to the contained executable or image stays with lanes B and D. Worker inputs carry no program paths. |
| Build plan slice 3, existing workspace graphs | `crates/server` imports the old registry and runtime, `crates/machine` imports its space channel, and current npm plugins import the original SDK. Removing those APIs now would break workspace consumers outside this lane. The Rust `operations` feature and Node `calternal-operations` condition select the new transport and SDK. No adapter translates the old protocol into the new one. |

## Fixture lines for the fixture owners

- `plugins-v2/calternal-chat/operations.ts` should accept the selected
  harness in `SendInput` and call its declared start operation with
  the newly written record's link. Its manifest needs the corresponding
  authorized cross-plugin call declaration under the host's slot rules.
- `plugins-v2/calternal-chat-claude-code/operations.ts` should accept the
  prompting record link in `StartInput` and pass it through the approved
  run input mechanism. Today it starts a session with only the chat ID.
- `plugins-v2/calternal-chat/kinds/chats.md` still uses a timestamp for
  the read cursor and unread count. The fixture owner should update both
  when the format engine accepts the anchor type and count expression.
- The fixture owners should use the shared typed operation-input
  declaration for the composer and harness slots and bind the harness
  program ID to its executable or image when those grammars are wired.

## Security questions for the coordinator

- **Worker containment and reuse (§8.2, primitives-only host T1–T3).**
  The process supervisor clears its inherited environment and owns the
  pipes. The test launcher uses Node's permissions with disposable code.
  Before enabling installed code, which OS sandbox enforces zero ambient
  network and filesystem access, bounded CPU and memory, and descendant
  termination? A plugin retaining one person's result in module state
  could expose it in a later person's call. Reusing processes across
  people saves cold starts, while recycling on actor change reduces
  retained state and still permits one live worker per installation.
  Keep external code disabled pending the sandbox and actor-reuse decision.
- **Pipe budget and admission (§8.1–8.2).** An allowed 32 MiB read can
  exceed 64 MiB when its text needs escaping. The current frame limit
  refuses the oversized result and keeps buffering bounded. Raising it
  increases worst-case memory per active worker. Chunking adds a framing
  state and preserves large reads. Decide the production frame and
  resource budget with the sandbox and route owners. Keep the bounded
  mock behavior for the slice.
- **Gesture-authorized harness choice (§7.4, §8.1 and eleventh set).**
  A dynamic harness is chosen by the person but `s.call` can call only
  declared operations. Does a trusted slot selection admit the selected
  provider's single start operation, or will Chat list each supported
  harness? The former allows third-party harnesses with a narrow grant.
  The latter is simpler and limits the list to known harnesses. The mock
  admits one explicitly known harness. The frame grants no slot authority.

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
`claude@calternal-dev-01-compute02` under `~/bin/lock-shared nix develop -c`.
It ran Cargo formatting and plugin-host tests with the `operations`
feature, SDK tests and strict TypeScript checking of both untouched
revision 9 fixtures. The process tests cover actual worker reuse and
restart, stale and malformed frames, Chat send and rename, receipt
replay and conflict, a crash before mock commit, and both undeclared
operation and reaction-run refusal. The send-and-start test uses a
synthetic Chat handler and the upstream harness.

The workspace gate ran under `~/bin/lock-full nix develop -c`. It checked
all workspace targets and features and the standalone conformance
manifest, built the bundled plugins and generated Rust and TypeScript
references. The checked-in public site needs 85 regenerated pages
under `apps/docs` (84 SDK pages and one crate page). Regeneration was
verified in the disposable VM checkout. Those files belong to the
integration path and are required for the clean-checkout docs gate.

## Simplification opportunities

- Move the ten caller-safe codes and `next` into shared types once the
  new call route is the workspace default.
- Remove the v1 registry, worker entry and export condition together
  when the server, machine and bundled preview plugins migrate.
- Derive slot, operation and event registration metadata from the shared
  install declarations and drop the transport's need to learn it.
