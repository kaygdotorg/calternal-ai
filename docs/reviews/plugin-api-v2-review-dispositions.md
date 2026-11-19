# Plugin API v2 review dispositions

## Sol review of revision 3 (revision 4 responses)

See the [second-round response table](https://git.kayg.org/kayg/calternal-ai/src/commit/cca3d12db6cb2f9b5f08252df2b7d8e3672d19be/docs/plugin-api-v2.md#second-round-review-responses)
for earlier decisions. Part B refers to the renderer specification in this
same tree. Test gates in §8 are requirements for later implementation.

| # | Resolution |
| --- | --- |
| 1 | Part B defines strict variant shapes and action uniqueness. §§4.1, 8 require the same checks at host activation. |
| 2 | §4.2 and Part B cache rules keep authorized rows in session memory and require fresh authority to paint them. |
| 3 | §3 and catalogue retain rename status and protected Undo. |
| 4 | §3 retains one recovery-visible fence for commit and invalidation. |
| 5 | §5 retains narrow guest descendants and denies guest jobs pending proof. |
| 6 | §3 and manifest retain typed redemption against fixed sites or actor-configured servers. |
| 7 | §8 retains the real shared writer stall and range/staging checks. |
| 8 | §8 labels the checked-in guide v1, excludes it from v2 trials and requires the rewritten examples. |
| 9 | §4.1 authorizes all dependency edges, rejecting cross-bind query operands in the first slice. |
| 10 | §4.3 requires independent resolution, host confirmation and fenced dispatch. |
| 11 | §3 retains the authoritative content/outbox transaction and recovery ordering. |
| 12 | §3 retains one staging reservation per checked receipt and bounded cleanup. |
| 13 | §§4, 8 add a real combined activation, approval and live-bind gate before freezing those surfaces. |
| 14 | Part B retains shared Bind, row template and placement. |
| 15 | §4.3, catalogue and Part B require activation identifiers, context and typed source scope. |
| 16 | §4.2 and catalogue specify typed draft, attachment, mention and quote refs. Component variants are in Part B. |
| 17 | §4.4 and catalogue specify status polling and protected purge inputs. |
| 18 | §3 retains hosted calendar, missed-run and notification semantics. |
| 19 | §7 retains `defineView` and `defineChrome` for exercised schemas. |
| 20 | §8 gates the rewritten v2 guide and compiled examples. No runtime or guide implementation in this revision. |
| 21 | §3 and Q9 record the Owner’s decision: TCP comes later under “Servers you add”. |
| 22 | §4.1, catalogue and Part B retain authorized viewer/route operands. |
| 23 | §§3, 4.1 and catalogue register canonical kinds and Trash/inbox fields. |
| 24 | §3 retains metadata save against the live room head. |
| 25 | §4.4 and catalogue map lifecycle/Turns actions to host dispatch without waking a plugin worker. §8 tests wake counts. |
| 26 | §§1, 3, 4.1 retain one method list, one approval resolver and scoped Bind references. |
| 27 | §8 requires executable Notes/Threads catalogue fixtures, cold read and a Mail paper walk before their gates. |
| 28 | §4.1 and catalogue define kinds, typed fields, owners, audience and the private Request ref. |
| 29 | §4.4 and catalogue give exact built-ins, routes, authority, arguments, outcomes and wake behavior. |
| 30 | §§3, 4.3 derive preview/confirmation/retry from host effects and checked idempotency, including material at egress dispatch. |
| 31 | §4.3 defines four identities, retention and replay fingerprint. |
| 32 | §4.1 and catalogue define predicate operands, count modes, half-open bounded windows and unsupported-plan errors. |
| 33 | §4.2 and catalogue define transcript, draft/quote and sealed frame scopes, retention, paging and sandbox/navigation. |
| 34 | Combined tip pins both parts. Manifest uses the shared schema and §8 gates host-loader parity with strict paper fixtures. |
| 35 | §4.3 requires the host context and server-side reconstruction. Only declared typed interactions leave the device. |
| 36 | §4.3 and Part B include untouched defaults, autofill and offscreen material and disallow client typing flags as proof. |
| 37 | §4.1 rejects cycles, pins search to the approved corpus and limits guest counts to count-enabled shared projections. |
| 38 | §4.2 and catalogue cap aggregate subscriptions, bytes, pending windows and work. Host prefetch/push never wakes plugins. |
| 39 | §4.3 uses stable target IDs and per-action dependency versions. §8 requires success during unrelated churn. |
| 40 | §4.4 and catalogue limit approval/Undo/share/purge to their protected surfaces and derive swipe eligibility. |
| 41 | §4.2 and Part B keep filled rows session-only, clear on invalidation and require authority for fallback. |
| 42 | §8 sets a five-second lease expiry bound and range pin behavior, and races invalidation across real commits and egress. |
| 43 | §8 tests deepest qualified plans, count churn/fan-out and committed transcript replay after restart. |
| 44 | §8 adds real host/renderer fault injections and keeps schema-only/author evidence insufficient for Invoke freeze. |
| 45 | §8 and Part B gate app migration on Share, bulk Notes, Turns provider/composer controls and executable fixtures. |
| 46 | §8 and Part B preserve custom snooze and require shipped workflow parity or Owner-approved exceptions. |

## Opus review of revision 4 (revision 5 responses)

API below means the [runtime and author API spec](../plugin-api-v2.md).
The renderer revision shares this tree. Prototype and timing gates are
requirements, with evidence to be collected in Step 0.

| # | Resolution |
| --- | --- |
| 1 | API §4.3 and the catalogue deliver view and stable row contexts, renewing them with the subscription lease. |
| 2 | API §4.1 and both schemas share catalogue WindowRequest/Result, including bind, state, typed count and stable row IDs. |
| 3 | API §§4.2–4.3 and both schemas compose Send from checked Thread/context, draft and issued options, with selected/default values. |
| 4 | API §4.4 and the catalogue add created refs, route-on-success, Turns create/archive/unarchive/snooze/follow-up, and the indexed snooze field. |
| 5 | API §4.4 and the catalogue take scalar metadata field/value and the context revision, with eligible boolean swipe and Undo. |
| 6 | Step 0 entry and the renderer verifier check fixture manifests, references and swipe authority. |
| 7 | API §§1, 4.1 and the manifest share catalogue KindId, field types/audiences, icon and title field. |
| 8 | API §4.1 makes generic fields reusable. Notes owns pinned; host owns Threads, Collections and Requests. |
| 9 | API §§1, 4.1 and the manifest declare Bind reads at install and check them before viewer authorization. |
| 10 | API §4.4 and both schemas type protected Undo, status, card resolve and Share/Trash sheets, with kind and role visibility checks. |
| 11 | Q10 leaves persistent own-space rows open. §3 keeps declared anonymous images separate and withholds the No network badge for content fetch. |
| 12 | API §3 and the catalogue type one-date/recurring calendars and notify_creator. Renderer chapters reconcile stale prose, routes and examples. |
| 13 | API §§1, 4 and the schemas use one material-label declaration, host option route, outcome union, Bind registry and canonical artifact namespace. |
| 14 | API §§1, 4 and manifest type trusted titles, material labels, created kinds and single/bulk targets. Palette commands are deferred. |
| 15 | API §4.1 and the catalogue scope self counts and Stop to their actual audiences. |
| 16 | API §4.3 derives confirm mode and admits one-exchange reversible actions with host-proven Undo. Awaits the final Sol check. |
| 17 | API §4.3 binds prepared Send to material and the receipt to the final text. Awaits the final Sol check. |
| 18 | API §4.2 and renderer spec apply same-version prefetch immediately and flush selection before activation. |
| 19 | API §4.2 stops paint at lease expiry and restores unchanged memory rows after verified renewal. Own-space persistence remains open and the relaxation awaits Sol. |
| 20 | API §§2, 8 specify pending after three plugin seconds, one status ref, a separate measured wake allowance and the cutoff test. |
| 21 | API §4.3 and the catalogue accept bounded stable IDs inside bulk resolve and return a checked selection ref. |
| 22 | API §6 and both schemas cut unused surfaces, use one Bind registry and move review dispositions into docs/reviews. |
| 23 | API §4 authorizes bounded Choice-from-Bind and Move to collection. Q11 leaves nested child rows and user-made sections/drag open. |
| 24 | API §8 puts executable fixture manifests at entry and names the renderer, locked perf VM, run count, percentiles, phone exchanges and keyboard gates. |
