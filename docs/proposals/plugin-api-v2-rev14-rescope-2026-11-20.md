# Plugin API v2 revision 14 rescope · design note

The server owns access, isolation, durability and work that must happen while a
space sleeps. Packages own file formats, behavior and presentation. Bundled and
community packages use the same manifest, broker and installation rules.
This pre-decision note proposed how to apply the [owner’s second set of answers][answers] to
[revision 14.15](../plugin-api-v2.md). The [third set of answers][third-answers] supersedes its open recommendations, including the sleeping view, editor choice, alarm delivery, offline expiry and search index.

## A. Harness-owned files and transcripts

**Leaves the server.** Delete the conversation-kind capability and built-in
Threads corpus, the canonical transcript writer and record format, the common
provider display envelope, the transcript page/follow and attachment projections,
the dedicated composer Send and Resume semantics, and the conversation-specific
parts of the fence table. Retire the native Turns dispatch and bundled namespace
exception at cutover. The orchestrator owns the Threads list, composer,
queueing, status presentation and the Resume choice. Each harness writes its own
thread files, implements provider controls and continuation, and supplies the
app’s agreed transcript presentation. The bundled packages can put their existing
provider decoders and legacy thread-reader logic there. Files stay readable during
migration. Verify committed text, headings, attribution and attachments against
the old reader before switching writers with zero live attempts.

**Remains in the server.** Use two generic surfaces:

- **Manage an execution.** Admit a declared job with an exact package digest,
  actor, item, placement, authority and quotas. Issue an attempt lease and
  revocable broker capabilities. Reuse durable jobs, protected approvals and
  receipts. Keep generic waits, Stop, child reservations and checked effects.
  Record the target and authority generation under the same fence that orders
  access changes and effects. Preserve exact-attempt reattachment, runtime
  freeze, durable teardown and restart reconciliation. These protect a job that
  writes any kind of item. The host need not know a turn phase or a provider
  session ID. [Existing attempts and adapter behavior][turns].
- **Append and read an item file.** A checked writer appends bounded opaque byte
  batches to its declared file slot, with an expected item revision and request
  ID. Bind any attempt-scoped writer to the exact attempt and audience version.
  Commit the revision, byte boundary and receipt together. ACK only after the
  bytes and boundary are recoverable. Replay the same receipt after a lost ACK.
  Reuse the staged writer, outbox and pinned-range model. An authorized reader
  opens a revision and requests bounded byte ranges through the host while the
  runtime sleeps. Recheck access before delivery and invalidate pins on revoke
  or purge. Generic file and attachment refs retain independent authority.
  A file-change feed carries revision invalidations, and the plugin assigns
  record IDs and turns byte ranges into pages. [Content publication][content].

**What the person sees and what it costs.** A basic row and item shell render
immediately from checked host data. Cold reads can open or download the file, and
a registered plain-text or Markdown file can use the generic file preview. For
the rich transcript, the shell shows its header and a loading state until the
harness supplies a validated native description. The app can request older pages
or follow changes from the harness under that view’s lease. The harness can batch
file appends and update delivery during a live session. A stalled viewer cannot
back up writes. This moves rich first paint onto the wake and decode path.
The 256 MiB snapshot fixture on perf-01 measured **269 ms p95 cached and 489 ms
p95 evicted** for a single validated settings reply (100 wakes per arm, before
explicit reseed). The merged dirty snapshot with reseed measured **469 ms p95
cached and 993 ms p95 evicted** (five wakes per arm). Neither fixture measured
transcript paint. Measure rich first paint separately, including backlog and
concurrent wakes. [Space runtime wake measurements][wake].

**Rules that survive the move.** Apply these in the host’s generic item and
execution authority paths, for any package using these primitives:

- **Audience widening, including Undo.** If a share, role or group change,
  invite, collection change, restore or Undo adds readers, an authorized sharer
  confirms a host-written disclosure that the new readers can read the whole
  committed file, including anything an agent found or wrote using private
  access. A widening inverse uses protected resolve and confirm in place of
  direct Undo. The host pins the checked audience and file scope and applies
  the same fence for each entry point. It neither parses records nor screens
  the source of committed text. An attachment read still checks its own grant.
- **Personal authority and saved sessions.** Before expanding access, durably
  fence personal-authority attempts and pending intents, discard uncommitted
  output and retire incompatible saved continuation bindings, including for
  completed runs. Bind an opaque continuation ref to the admitted item,
  audience, actor, provider contribution and profile. Invalidate its generation
  on widening and refuse retired refs even if the harness offers Resume.
  Later shared runs can start from the committed file and context cleared for
  that audience. Shared runs must be unable to reopen personal provider history
  by path or by a native session ID. Admit shared runs only on placements that
  can enforce that boundary. Apply the shared discovery and tools
  rules from the authority decision at admission and delivery.

**Open for cutover.** Is the generic file preview plus Loading acceptable as
the sleeping transcript? A plain preview opens without a wake, while the rich
timeline adds the measured wake and decode delay. An optional plugin-produced
view snapshot would make the rich view faster to open, at the cost of duplicate
state, invalidation and a new protected projection. Start with the file preview.
Pin the Threads attribution, attachment and queued follow-up behavior before
cutover, as agreed. Qualify append/replay and stale-session fencing on a real
harness before removing the old engine.

## B. One Threads list and app contributions

**Leaves the server.** The orchestrator defines the Threads app and the harness
contract. Its list can include both `calternal-orchestrator-claude-code` and a
community harness. Another orchestrator can use the same harness interface or
publish its own. The server carries no built-in list of harnesss or Notes editors.

**Remains in the server.** An app manifest declares versioned extension points.
Matching installed packages contribute a provider or harness, a panel, a row
field, or an action through the same mechanism. The host resolves compatible
versions, pins package digests, checks requested access and installation consent,
and dispatches a contribution with the caller’s current rights. Ownership of an
app grants no access to the contributor’s files, and registration grants no
right to modify another app. Use normal host validation and native rendering for
contributed descriptions. Propose one app-owned list entry kind with a checked
source ref per entry. Each harness contributes entries for its own files through
the declared interface. The index scopes rows and counts to source authority
before windowing. The app draws base rows immediately and fetches optional
fields only for visible rows, bound to the source revision and audience.

Use declared format and version IDs for files. Two notes editors can opt into
the same Markdown format and use the same host item, revisions and edit fences.
Packages can also use private format IDs for their own notes. Format support
does not grant access or permission to rewrite a file. [Manifest and renderer boundary][description].

**Cost and open choice.** Extension contracts need compatibility tests, clear
missing-contributor behavior and rules for overlapping contributions. For
example, two editors may both support a shared note, and one may understand
fewer blocks. Recommend explicit preferred-editor selection and format-level
read/write claims. Choosing automatically saves a tap but can change how a file
opens after install. An editor must preserve blocks it cannot edit or open
read-only. Confirm the first shared Markdown profile and which plugin
maintains the initial harness interface.

## C. Durable alarms

**Leaves the server.** Calendars, time zones, interval and recurrence rules,
missed-run policy, retry cadence and recurring schedule UI live in packages.

**Remains in the server.** Propose a one-shot durable alarm for a declared job and
absolute time T, with a bounded pending count per package and space. Replace or
cancel by checked ref. A due alarm consumes its record and admits one durable
job in the same transaction. The host can replay that job under its existing
receipt without firing the alarm twice. Restore pending alarms after restart and
recheck enabled package generation, live principal and quotas before admission.
On disable, retire outstanding alarms and fence claimed work. On re-enable
the package can reconstruct future alarms from its own schedule files.

**Cost and open choice.** One write to arm and a consume/job commit per firing.
The plugin must durably calculate its next time and handle clock changes,
daylight saving, late execution and a crash before rearming. Decide whether a
missed alarm fires once after restart or expires. For example, a weekly
notification due while the host was down could arrive late or be skipped.
Recommend delivering an eligible alarm once with both scheduled and actual
times, so the package can skip or act on it without a recurrence rule in the host.

## D. Full offline content

**Leaves the server.** Apps decide how to render their downloaded formats and
which content people can pin or prefetch. Replace the sealed restriction on
persisting body-derived row parts with a rule for an authorized offline copy.

**Remains in the server.** An authenticated device can download every readable
revision component selected for offline use, including bodies, body-derived
fields and authorized attachments. Bind the manifest of refs and revisions to
the account, device, space and authority scope. The client encrypts it at rest,
keeps it separate by account, and uses fresh host checks for live actions.
On revocation the host stops issuing bytes and sends invalidations. On next
contact the client wipes revoked bytes, thumbnails, extracts, indexes and their
keys. A disconnected device can keep previously downloaded data until it
reconnects. A person who has read or exported a file can retain a copy.

**Cost and open choice.** Storage, sync bandwidth and invalidation grow with
the downloaded content. A short reconnect deadline reduces the offline
exposure window for managed clients but can lock someone out while traveling.
Recommend explicit download controls and per-device storage budgets. Confirm
whether signed-out local copies are ever retained and whether a space can set
an offline expiry. Export should start with files and attachments under their
individual read grants.

## E. Default and plugin ranking

**Leaves the server.** Provider-specific ranking, preference signals and app
ranking policy belong to the package that offers them.

**Remains in the server.** Ordinary queries always run the host’s bounded,
audience-scoped ranking without a plugin call. An app that chooses a custom
ranker can explicitly request a bounded set of already-authorized candidate
refs and fields. Run the ranker off the default path with a deadline, validate
its returned permutation and keep it pinned to the checked generation and
audience. Recheck authority on delivery. Show the default results while custom
ranking is pending. Keep its order stable across windows. Use no exact count
for an incomplete candidate set.

**Cost and open choice.** A bounded rerank can improve order inside a candidate
set. A package-owned index could find candidates absent from that set, at the
cost of background indexing, freshness work and a separate authority proof.
Start with reranking and measure it. Decide whether a later custom index is
needed when an actual package can show missed results.

## F. Community harness freedom

**Leaves the server.** Drop bundled harness IDs, namespace overrides, provider
selection and first-party execution exceptions. Community and bundled harnesss
can declare images, network destinations and a native login in the person’s
own sandbox under identical rules.

**Remains in the server.** Verify the installed digest and place the run in
the admitted sandbox. Recheck actor, item refs or expressly authorized
discovery, effects and quotas on each broker call. Apply network policy at
egress. Disclose the image publisher and digest, placement, mounted data,
provider-profile access and network destinations at installation or when the
approved scope changes. A distinctive provider mark requires a verified
publisher binding. A mark makes no claim about safety. Keep the shared-space
tools restriction until a boundary is proved, regardless of publisher.

**Cost and open choice.** Package code or tools running as the person can read,
alter or export that person’s provider login and data made available in the
sandbox. Malicious images can run miners, try to escape or impersonate a
provider. Quotas, restricted mounts and egress checks limit reach. They do not
protect a login from code allowed to read it and contact an approved destination.
Decide which placements a user can select at first release. A personal enrolled
machine puts the risk with its owner. A hosted sandbox adds operator risk and
needs a measured isolation proof. Use the same qualification suite and disable
unqualified placements for every publisher. [Existing credential probe][credentials].

## G. Trash and cutover order

**Leaves the server.** The mention picker, snooze presets and custom date
picker belong to the relevant plugin and may follow cutover. App-specific
Trash and retention rules leave with the native app engines.

**Remains in the server.** Keep one authority-checked Trash in each space with
fixed 30-day retention, generic restore and bounded purge that works while the
runtime sleeps. A restore that adds readers uses the same widening action.
Push notification text uses the authorized title and a fixed phrase.

**Cost and open choice.** Trash holds bytes for 30 days, so quota and purge
must account for large items and attachments. Draft the generic item and file
contracts first, then build **orchestrator → Notes → Mail → Files → Photos**.
The Notes cutover pins history browsing, comments and collection creation.
The Threads cutover pins old-message attribution, attaching files and queued
follow-ups. The Photos decisions wait for that plugin. Review the cutover
qualification sequence and confirm which large-file restore and purge arms
are required before switching Files. The 30-day rule and build order are settled.

[answers]: https://git.kayg.org/kayg/calternal-ai/src/commit/37cadea0/docs/morning-2026-11-20.md#second-set-of-answers-owner-2026-11-20
[turns]: ../agent-turns.md
[content]: ../../crates/content/src/publication.rs
[wake]: ../measurements/space-runtime-walls.md
[description]: ../description-contract.md
[credentials]: https://git.kayg.org/kayg/calternal-ai/src/commit/b3964ef3/docs/measurements/provider-credential-brokering-2026-11-20.md

[third-answers]: https://git.kayg.org/kayg/calternal-ai/src/commit/18e467b6/docs/morning-2026-11-20.md#third-set-of-answers-owner-2026-11-20
