# Threads app spike · 2026-11-20

The lane starts from dev `80864d0d`. The spike has rerun against
14.30 (`b89586dd`), 14.31 (`00828198`), 14.32 (`3ca11613`) and
14.33 (`47024c0f`) and 14.34 (`b4d0f20d`). The current pin for both
packages is draft 14.35 at `36d49713164ee0c56b8c1d3ffebfc844fd8b44cf`
on `origin/lane/v2-host-base`. It follows the November 20
decision that the Threads app (`calternal-threads`) owns the conversation file and the selected harness
owns its working log. The lane issue is [#92](https://git.kayg.org/kayg/calternal-ai/issues/92).

Threads creates one Markdown item per conversation, in a readable dated
folder. Its frontmatter holds the stable conversation ID and people and
agent participants. Markdown message headings hold stable
message anchors, with the harness-owned log reference on the reply. Send
admits the checked harness and placement through one replayable job. Send
while a job is active uses the same admission for the single successor.
The background handler appends the reply. Stop and archive go through
generic host controls. Rename updates the document and indexed title.
Snapshots contain complete messages within the native description limits.
The live read uses the requested revision and a private older-page cursor.
An execution alarm keeps the checked harness and takes the default
placement.

## Evidence and reproduction

Run on dev-02 in the lane directory with the draft commit available
locally and the dependency cache populated.

```sh
export TMPDIR="$HOME/Developer/lanes/threads-plugin-tmp"
mkdir -p "$TMPDIR"
flock -s /tmp/calternal-dev02.lock nix develop -c bash -c '
  npm ci --offline --ignore-scripts --no-audit &&
  npm run build -w @calternal/plugin-sdk &&
  npm test -w @calternal/threads &&
  npm run conformance -w @calternal/threads
'
```

The build uses the 14.35 revision-aware SDK builder. It pins both
manifest/worker packages, the Threads template and the harness entry.
Tests load the emitted artifacts and validate the fake's typed exchanges.
All 38 tests and both Conformance tool 1.1.0 package checks passed
on dev-02 at the 14.35 pin under the shared lock. The repository
documentation gate passed in the earlier afternoon run. The verified entry asset still uses scripted
output in the fake. Live installed placement remains unqualified.

The agent-thread tests exercise create, first Send with the contributed harness,
reply and linked log, preservation of a stopped run’s partial log, rejection of stale
Stop, checked selection, queue capacity and promotion after teardown,
archive retirement, rename during output, bounded snapshot paging, late
and on-time alarm delivery, receipt replay after a lost append ACK, and
distinct paths for matching titles. Its Claude Code output is scripted.
The fake does not prove real session, authority, crash recovery, or
placement isolation. Step 0 issue [#78](https://git.kayg.org/kayg/calternal-ai/issues/78)
documents the revision 13.5 renderer fixture and qualification probes.

## Findings

| Area and reference | What the plugin needed | Smallest change |
| --- | --- | --- |
| File custody, API §§1 and 3 and description §9.3 | The latest owner decision assigns a conversation to Threads and a log to the harness. §1 and the worked exchange assign the conversation to the harness. A harness that forks to its own writer would also bypass Threads' participant and message rules. | Update the worked exchange to admit a job against a Threads-owned item, deliver a harness result, and append from the Threads owner under the exact attempt fence. |
| Harness ingress and egress, §1, §3 and catalogue `extension_points`, `execution_engines` | `ExecutionJobStart.input` goes to the owning job. `ExecutionStart` accepts harness and continuation. Nothing specifies how a checked prompt reaches the contributed operation or the provider entry, or how reply bytes and a log link get back to the owner. Status and wait carry phase, approval, child and terminal signals. | Pin a generic contribution handoff with typed input and bounded, ordered output tied to the exact attempt, plus terminal and replay semantics. Define how each owner commits its own file under that fence. A completed one-shot result suffices for the first slice. Streaming needs the same checked route. |
| Harness entry and packaging, §1, §3 and manifest `harness` | The image digest and entry asset are declarative pins. Authors need the executable ABI, image acquisition and verification, invocation, cancellation, return values, log mount or broker access, and where execution egress is described apart from the ordinary worker's network policy. | Publish the entry ABI and a same-path example with a reviewed image, resolved publisher, mounted profile and explicit placement/egress disclosure. Teach the offline builder to include and verify entry assets. |
| Contribution target identity, §1 and manifest `contribution.app` | The prose resolves an exact app and both package generations. The manifest only names the package-local app ID as `app`. Different packages can plausibly expose the same ID. | Qualify the app by its owning package, or pin and specify a globally unique app namespace and collision rule. |
| Item create, save, range and placement, §3 engine surface and staged save | Common item methods have prose but no catalogue request/response schemas. The create needs a checked item ref, a revision, and a readable dynamic child folder with collision behavior. The kind's folder is static. The worker also lacks an audience-safe display name for the person's part of the readable path, so the spike uses date and title. Staged save describes begin/chunk/seal but the author method enum has no precise staged calls. | Add typed create, save, head and range, including safe relative path components under the kind's declared root, atomic collision handling, a typed head conflict, and typed staged save arms. Specify audience-safe naming inputs for placement. Use the existing content writer. |
| Rename and lifecycle projection, API §§3 and 4.4 and catalogue `host.item.rename` / `host.item.archive` | Rename of the app title can update the Markdown header with the indexed metadata, or generic Rename can update placement and Undo. No declared operation can combine both. Archive is generic and fenced, but the generic field table does not expose an `archived` list predicate. | Add a checked owner-authored body and title/placement update in one content receipt if folders should follow titles. Expose the generic archive fact to filtered Binds without asking owners to mirror it. |
| Worker context and receipts, §§3, 4.3 and 7, SDK `worker-v2.ts` | Worker code needs the live job and alarm metadata, stable parent request identity for subeffect IDs, stable actor attribution and an authored timestamp that survives replay. The declared host frame includes job and alarm, but no author-facing request, actor or stable clock context. The dev SDK worker accepts a v1 manifest, v1 host-call inventory and a separate fixture frame with no revision 14 context. | Generate a typed author context from the revision 14 carrier, exposing only stable author data and protected job context. Derive subeffect receipts from that context. Teach the SDK how to encode refusal status and retryable head conflict. |
| Create outcome, API §4.3 and manifest `operation.creates` | A worker-owned create must become an activation `created` ref for navigation. The generic outcome has the field and the manifest declares a kind, but their mapping isn't specified for worker results. The spike uses a `created` output. | Pin the output convention and require the host to check that the exact invocation produced the returned same-kind ref. |
| Stable identifiers and message links, API §§3 and 4.2, catalogue `view_snapshots` | The app can store a stable message ID and look it up in a checked item. `ItemViewRead` only accepts latest or older. Snapshot leaves have no message anchor and `ItemRef` / `HarnessRef` specify checked scope, leaving portable, durable identity for the log link and participant uncertain after reinstall, export or receipt expiry. | Define an authorized item locator and optional opaque message anchor on item-view open and leaves. Specify which IDs can be stored as portable references versus only used as live operands. |
| Alarm lateness, API §3 and `AlarmDelivery` | Timestamps distinguish an exact on-time admission from a delayed one. They cannot tell an outage miss from ordinary scheduler jitter, so the spike's strict comparison would skip normal late-by-milliseconds runs. | Define scheduler delay semantics, or add a host-attested missed-window fact while leaving the action choice with the plugin. |
| Snapshot text bounds, description `SnapshotText` and API §7 | JSON Schema `maxLength` counts characters. The pinned validator rejects a string of at most 2,000 multibyte characters when its UTF-8 encoding exceeds 2,000 bytes. The spike splits at 2,000 bytes and keeps a whole message together in its page. | Specify the intended unit and add a Unicode boundary fixture shared by SDK, tool and host. Keep the independent aggregate byte cap. |
| SDK and guide, API §7 | The dev guide and offline builder target the sealed contract. The author and renderer generators can consume draft schemas, but the sealed SDK build rejects draft manifests and packages only the older verifier module list. The public guide does not show execution jobs, alarm evidence, view publication or a cross-package harness. | Ship a revision-aware validator, package builder and broker context, and a walkthrough that runs the same plain app plus separate harness example on the real and fake hosts. |
| Conformance tool 1.0.0, `description_verification.py` | `check_manifest_install` unconditionally reads the catalogue's removed `conversation_capability` key. Both package runs crash after local manifest shape checks. The same module's worker-invocation check still keys on `execution_intent` while the draft job declares `execution`. | Update the tool to the revision 14 catalogue and add regressions for a plain `execution_target` app manifest, a separate contribution manifest and an alarm execution job invoke. |
| People-thread custody and participant refs, owner first answers and API §§1, 3, 5 | The worker can record checked people as participants in the existing kind. A list in Markdown carries no authority. The fixture supplies a prechecked account set and models one thread file with no group or cross-space behavior. | Pin the custody and membership choice before exposing people-thread creation. Define a checked operation for establishing the participant audience and file rights atomically, including which participant identifiers may be written or displayed. |
| People-message edits and tombstones, API §3 item save and §4.2 search, history and views | A participant can edit only their own message in the fixture. The same anchor resolves to an edited mark or tombstone. The previous revision can still have the removed text, and the whole-file rewrite must remain within inline limits until staged writes exist. | Specify what deletion promises for live views, retained pages, history, search excerpts and offline copies. Reuse revision-fenced staged save. Add targeted invalidation if ordinary replacement is insufficient for the deletion promise. |
| Private per-person thread state, API §3 item methods and §4.2 offline state | Package-owned shared items and hidden placement do not establish actor-private storage. Draft, mute, and read position need item-scoped, account-private persistence, writes from multiple devices, and cleanup independent of the shared transcript. | Pin an actor-scoped read and compare-and-set write for private item state, or explicitly give ordinary items account-private ownership. Publish applicable quotas, offline, receipt, sign-out and erase behavior. |
| Scope and consent for called-in agents, API §3 every call and delivery, §5 and catalogue execution admission | The fixture binds an invitation to the inviter, item, source revision, selected message/range/whole scope, and consenting message IDs. The inviter authorizes their own selected messages. An old grant cannot leak new edits. A normal Send and alarm must load the Markdown to reject admission before creating a job. | Add host-issued consent and invitation bindings with visible author, exact selected message revisions, expiry and revoke rules. Revalidate at admission, before data delivery to the exact attempt, and when committing its reply. Pin a body-independent admission rule for people-only threads and a replayable one-use invocation receipt. Specify whether unrelated edits should invalidate the whole grant or only the affected messages. |
| Called-in agent data and output, API §3 attempt authority and catalogue item/query/view routes | Delivering a filtered prompt is insufficient if an agent could subsequently read the full item through item ranges, history, snapshots, search, list or tools. The fixture only delivers selected human message IDs and text, and hands the reply back to Threads. | Add an execution read ceiling for a people thread that all alternate read routes honor. Let the attempt consume a bounded approved projection. Recheck audience/consent independently of what the ordinary inviter may read, and link the harness log through a separately checked reference. |
| Placement for a private people conversation, API §3 and catalogue `execution_placement` | The owner decision uses the inviter's login and machine in a private people conversation. The current generic placement examples distinguish personal spaces and shared-space login choices. File custody for people threads is still open. | Pin placement and profile derivation for a checked private people-thread invitation. Keep shared-space subscription behavior on the existing path. |


### Revision 14.31 disposition

CLOSED means the draft now supplies the missing contract, schema or tool
behavior. Real host qualification remains under Needs host.

| Original finding | Status | 14.31 evidence or remaining gap |
| --- | --- | --- |
| File custody | **CLOSED** | API §§1 and 3 and description §9.3 use a Threads-owned conversation and a separate harness-owned log. The fixture writes through those two owners. |
| Harness ingress and egress | **CLOSED** | `execution_start`, `contribution_call`, and `contribution_read` pin the checked handoff, attempt writer scope, ordered output and terminal. Tests consume both frames and stop the same attempt after the first committed reply. |
| Harness entry and packaging | **PARTLY** | `extension_points.harness_abi` specifies entry frames and pins. The SDK packages the entry and checks its digest. The manifest has no execution egress or OCI source field despite the ABI prose. The live broker endpoint and real image lifecycle remain unqualified. |
| Contribution target identity | **CLOSED** | The qualified app is `{package:"calternal-threads",app:"threads"}`. The fake verifies both components and equality of the two interface declarations. |
| Item create, save, range and placement | **CLOSED** | The catalogue now has typed create, head, save, range, staged operations and `head_conflict`. Relative name/folder placement, collision bounds and the optional audience-checked actor label are specified. The spike exercises inline operations and collision replay. |
| Rename and lifecycle projection | **PARTLY** | A single save can update body, title and leaf name. The generic `archived` predicate is specified. Moving the original folder on title change remains open. |
| Worker context and receipts | **PARTLY** | `AuthorInvocationContext`, the SDK adapter and typed conflict replies supply the stable actor, clock and request ID. The fake uses the adapter. Live carrier dispatch and some generated broker shapes remain incomplete. |
| Create outcome | **CLOSED** | `created_outcome` and tool checks bind the closed `{created}` result to a same-invocation create receipt. The internal log uses `item_create` without `creates`. |
| Stable identifiers and message links | **PARTLY** | `ItemLocator`, `item.locate` and `ItemViewRead.anchor` now have contracts. The spike stores the locator and verifies anchored pages and missing anchors. A portable logical harness participant identity across a changed installation binding still needs a source. |
| Alarm lateness | **CLOSED** | Authored `grace_seconds` and host-stamped `missed` distinguish delay within grace from a missed run. The worker uses `missed`. The tests cover delivery one second late inside a 30-second grace window. |
| Snapshot text bounds | **CLOSED** | Tool 1.1.0 counts Unicode code points and includes a Unicode regression. The spike uses code-point leaves within the unchanged aggregate byte budget. |
| SDK and guide | **PARTLY** | A revision-aware builder, context adapter and two-package walkthrough are available on the draft branch. The dev SDK and installable guide remain sealed-revision artifacts. Real/fake parity is unqualified. |
| Conformance tool crash | **CLOSED** | Tool 1.1.0 accepts both real packages and the producer regression validates the current `execution` job declaration. |
| People-thread custody and participant refs | **OPEN** | Custody, trusted authorship and atomically establishing a participant's file grants remain open. The fixture provisions checked participants. |
| People-message edits and tombstones | **PARTLY** | Atomic content save and the message anchor route are now defined. Deletion behavior for history, search, cached pages and offline copies is still open. |
| Private per-person thread state | **OPEN** | There is no actor-private, item-scoped compare-and-set or retention contract. |
| Scope and consent for called-in agents | **PARTLY** | `attempt_disclosure` pins consent to included authors, attempt, immutable source and purpose. Trusted scope resolution, proof recording and revocation remain open, and the affected invocation stays disabled on the real host. |
| Called-in agent data and output | **PARTLY** | Kind-wide denial covers alternate access paths and the contribution handoff fences writes. Item-level screening and a trusted consent-filtered projection are missing. |
| Placement for a private people conversation | **PARTLY** | The rule specifies the inviter's enrolled machine and login in the private case and the space binding for the shared case. There is no checked invitation placement exchange yet. |

### Findings from the 14.31 rerun

| Area and reference | What the plugin needed | Smallest change |
| --- | --- | --- |
| **Discovery is too broad**, API §3 “Threads participants”, manifest `kind.attempt_discovery`, catalogue `execution_attempt_authority.discovery.kind_opt_out` | §3 says “Threads denies attempt discovery on its kind.” The catalogue applies that flag to every item of the kind, including a person's own agent threads, for search, list, direct reads and derived output. §3's preceding sender-only rule cannot restore access. The spike declares the deny flag to exercise the draft as written. The owner decided to deny by default for conversations between people. | Pin a host-checked item-level fact for conversations between people. Evaluate it before any discovery or direct read. Keep the ordinary sender-only rule for one's own agent threads, and test both against the same kind. |
| Harness egress and image acquisition, catalogue `extension_points.harness_abi` and manifest `definitions.harness` | The ABI says to acquire from a declared OCI source and apply a separate execution network declaration. The closed harness schema accepts only image digest, entry, name, icon and login directories. A community author cannot express either declaration. | Add the OCI source and execution egress fields to the closed harness schema and use them in install disclosure and launch checks. |
| Generated broker types, SDK schema-to-types | `ItemSave` generates only an unknown-key object. The successful `ItemViewPage` arm loses its view and cursor properties. This hides mistakes even though conformance schema validation works. | Preserve root properties when generating types for an object with required-only `anyOf` or `oneOf` arms. Add compile-time regressions for both definitions. |
| First Send target operand, description §9.3, manifest operation target and catalogue `ExecutionJobStart` / `AlarmSet` | §9.3 sends `input:{text}`. The verifier requires the run operation to target the execution kind, while the shared Send/alarm operation needs a checked target field. `AlarmSet` has no top-level item field. The spike carries `input.item` on both paths and checks it against `job_context.item`. | Define how an execution operation's declared target enters its input on either admission route, or update the worked input with the checked `item` field and explicitly require equality with the pinned target. |
| Encoded contribution limits, catalogue `extension_points.calls` and guide | A string allowed by the point's regular JSON Schema can exceed the 16 KiB frame once encoded, especially for emoji or escaping. The spike checks JSON bytes and reserves envelope room before sending. | Expose a shared SDK limit check with the encoded envelope bound and a caller-safe refusal before admission. |
| Participant identity, catalogue `item_engines.locators` and job context | The app must not persist `HarnessRef`. The spike stores a thread-local derived ID and can preserve that identity while the underlying checked binding stays the same. The job context gives no stable contributor identity across reinstallation or a new binding. | Expose a checked non-authorizing contribution identity to the owner if an agent participant should survive changes to its live handle. |

### Description §9.3 exchange comparison at 14.31

All 20 JSON Lines records in the personal, shared and broker examples
parsed and matched their selected 14.31 request and reply schemas on
dev-01. The comparison below distinguishes shape checks from fixture
behavior and remaining real-host evidence.

| Exchange | What the plugins and fake do |
| --- | --- |
| Prerequisite empty item | Matches ownership. The app tests `item_create` and an initial retained empty page. The example begins after the host has resolved a deep link. Locator resolution is still a host task. |
| `app.open` | The emitted descriptor has the item view and a composer with harness and placement slots. It also has the fixture Create, Rename and Archive controls. The verifier checked its template. The fake does not open a renderer session. |
| `view.open` | The example's view lease and activation context validated by shape. A live authenticated lease and role-specific issuance were outside the fake. |
| `extension.choices` | The fake resolves the qualified app and harness point, checks both interface declarations and includes an option and its checked harness ref. |
| `choice.options` | The fake returns a placement bound to that harness and rejects unchecked harness or machine refs. It did not run the second placement, real enrollment or account login selection. |
| Send `activation.resolve`, `activation.confirm`, `activation.dispatch` | All three paper exchanges matched their schemas. Worker tests begin at checked dispatch, after the host would turn the option into a `HarnessRef` and validate protected confirmation. |
| `start_job` | The worker checks that ordinary Send is eligible, then forwards the checked harness and placement. The fake returns a receipt and bounds the successor queue. The spike includes `item` inside job input as well as the top-level checked target. |
| First live `item_view.open` | The declared refresh handler reads a revision pin. The fixture can produce an empty page, then a prompt at the next revision. It supplies no literal “Starting” text. The renderer follow ref is host-owned. |
| `execution_start` | Matches protected job context and the exact harness. The fake reports `running` immediately, while the example first reports `starting`. |
| `contribution_call` | Matches the checked prompt and exact job/attempt. The fake creates one call and replays its receipt. It invokes the built harness worker with scripted output. |
| `contribution_read` and append | Matches `seq:1` output with text and an independent log locator. Threads appends its own reply and publishes a snapshot. `seq:2` terminal is read on completion. The replay test loses an append ACK and keeps one reply and one provider call. |
| `item_view.follow` and next `item_view.open` | The fixture tests the changed head, retained page replacement and checked anchor refresh. Renderer subscription delivery, catch-up and lease invalidation still need the host. |
| `execution.control`, then Stop resolve, confirm and dispatch | Paper shapes match. The fixture exercises the exact attempt Stop while the same job waits for its terminal frame. It preserves already committed text and log. Renderer contexts and protected confirmation are host-owned and unqualified here. |
| Shared-space replacement lines | All three replacements match their schemas. The fake is a single-space model. Shared default login, explicit personal login, audience intersection and the shared tools fence still need live qualification. |


### Revision 14.32 disposition

These statuses cover the 19 initial findings and all six findings from
the 14.31 rerun. CLOSED means the missing author contract or tool surface
is present. Live qualification is tracked separately.

| Finding | Status | 14.32 evidence or remaining gap |
| --- | --- | --- |
| File custody | **CLOSED** | Threads writes the transcript. A checked contributor writes its own log and returns an independently authorized locator. |
| Harness ingress and egress | **CLOSED** | The pinned broker calls still supply an ordered stream and exact attempt fence. The Stop and lost-ACK tests pass. |
| Harness entry and packaging | **CLOSED** | The ABI, asset packing, OCI source and separately approved execution network fields are declared. The lane's fixture source is `registry.example.invalid/calternal/threads-claude-code-fixture` with explicit `execution_network:"none"`. Real image acquisition, executable launch and egress enforcement still need host qualification. |
| Contribution target identity | **CLOSED** | Both package and app are qualified and the fake checks the two declared interfaces. |
| Item create, save, range and placement | **CLOSED** | Typed inline and staged methods, conflict result, readable placement and optional checked actor label remain defined. The spike continues to exercise the inline subset. |
| Rename and lifecycle projection | **PARTLY** | Atomic body/title/leaf rename and the archived predicate remain defined. Moving the folder with a rename is undecided. |
| Worker context and receipts | **PARTLY** | The adapter exposes the checked author, job and alarm facts, and the generated broker types have improved. Sealed dev worker dispatch still needs the draft carrier adapter for the live host. |
| Create outcome | **CLOSED** | The tool and catalogue require a checked `{created}` result. The harness creates its log internally without `creates`. |
| Stable identifiers and message links | **CLOSED** | Log locators, checked anchors and portable logical contribution identity are defined. Message authors keep one local ID across a fresh live harness binding. |
| Alarm lateness | **CLOSED** | The host records the grace-based missed bit. The worker tests both within-grace and missed runs. |
| Snapshot text bounds | **CLOSED** | Unicode code-point leaves and the independent aggregate-byte cap remain in use. |
| SDK and guide | **PARTLY** | The pinned SDK builds, encodes and validates both packages. The draft walkthrough is updated. The installable guide and real-host parity remain separate work. |
| Conformance tool crash | **CLOSED** | The verifier accepts both packages at the new pin. |
| People-thread custody and participant refs | **OPEN** | The fixture can provision participants. Production custody, trusted authorship and atomic participant grants still need decisions and checked routes. |
| People-message edits and tombstones | **PARTLY** | Typed saves and anchors work. Historical content, search and offline/cache deletion propagation remain open. |
| Private per-person thread state | **OPEN** | An actor-private item-scoped write, compare-and-set and retention policy remain unspecified. |
| Scope and consent for called-in agents | **PARTLY** | The item policy and checked-disclosure boundary are defined. Trusted selection, proof storage and revocation mechanics remain open. |
| Called-in agent data and output | **PARTLY** | The kind has a per-item cutoff across discovery and direct reads. The real host still needs the trusted per-author projection and consent path. |
| Placement for a private people conversation | **PARTLY** | The inviter's own machine and login are specified by policy. A checked invitation placement exchange is still open. |
| Kind-wide denial of discovery (14.31) | **CLOSED** | The same kind uses `item_policy`. A host-attested `creator_only` thread is discoverable only by attempts acting for its creator, under the normal audience checks. A `consent_only` thread stays out of discovery even after a one-message invitation. The fake verifies both on items of the same kind. |
| Harness egress and OCI acquisition fields (14.31) | **CLOSED** | The SDK and verifier accept explicit `image_source` and `execution_network` in the contribution. These are distinct from the ordinary worker network declaration. |
| Generated `ItemSave` and `ItemViewPage` (14.31) | **CLOSED** | The wire types retain the authored fields and union branches. The fake uses the generated `ItemSave` with its previous intersection removed. |
| First Send target operand (14.31) | **CLOSED** | Send and alarms place the target solely at top-level `item`. The fake refuses duplicate target fields and injects the checked item into the run operation's input and job context. The rewritten §9.3 now matches the broker requests. |
| Encoded contribution limits (14.31) | **CLOSED** | The SDK checks the complete JSON frame plus LF. The spike applies it to `ContributionCall`, entry delivery and output. An escaped prompt that only exceeds the encoded limit fails before the provider entry is called. |
| Participant identity (14.31) | **CLOSED** | The host stamps `{package,contribution}` into choices and job context. Threads stores that identity and derives a stable local author ID. The rebind test rejects the old live ref and proves the new checked binding uses the same participant. |

### Findings from the 14.32 rerun

| Status and reference | What the plugin needed | Smallest change |
| --- | --- | --- |
| **OPEN** · Preflight for encoded contribution size, catalogue `extension_points.calls` and description §9.3 | The exact encoder correctly protects entry dispatch. A direct Send can still receive a job receipt and persist the human prompt before a contribution frame is rejected. Full-frame validation needs the attempt ref, which the worker receives after admission. The escaped-prompt regression exercises that timing. | Specify a callable preflight or a conservative author input budget before job admission if the composer should reject this case synchronously. Keep the exact frame check at dispatch. |
| **OPEN** · Provenance for privacy downgrades, catalogue `execution_attempt_authority.discovery.item_policy` and opaque `item_save` / `item_append` | The policy requires downgrading before imported material of unproven authorship becomes discoverable. Writes carry opaque bytes. The host can attest the create and detect another authorized writer, and a caller can explicitly request `consent_only`. The schema carries no trusted import/provenance designation for bytes an owning worker writes on behalf of the creator. The spike creates fresh items under checked creator admission and keeps the people invitation path fixture-only. | Specify how generic imports and unproven authoring paths trigger downgrade in the fenced writer. Pending that rule, require consent-only for such paths. |


### Revision 14.33 disposition

The table revisits all 27 prior findings. CLOSED tracks the author or
contract gap. Live host qualification remains separate.

| Finding | Status | 14.33 evidence or remaining gap |
| --- | --- | --- |
| File custody | **CLOSED** | Threads still writes the conversation and the harness still owns its separate log. |
| Harness ingress and egress | **CLOSED** | Admission pins the exact input. The checked contribution transports bounded output and a terminal frame. |
| Harness entry and packaging | **CLOSED** | The declared ABI, asset pins, OCI source and explicit execution policy pass the package checks. The registry and entry are still fixtures. |
| Contribution target identity | **CLOSED** | The qualified app and both declared contribution interfaces still match. |
| Item create, save, range and placement | **CLOSED** | Typed operations pass. The fake now exercises revision-checked staged replacement as well as the inline paths. |
| Rename and lifecycle projection | **PARTLY** | Atomic body, title and leaf update and the archived predicate are defined. Moving a thread's folder on rename remains open. |
| Worker context and receipts | **PARTLY** | The adapter carries stable author context, protected job metadata and the pinned input. Sealed dev worker dispatch and live receipt recovery remain separate work. |
| Create outcome | **CLOSED** | The checked created item maps to the activation result. Logs use internal item creation. |
| Stable identifiers and message links | **CLOSED** | Anchor and locator tests pass, and an agent keeps its logical participant identity after a fresh checked binding. |
| Alarm lateness | **CLOSED** | The worker still applies the host-recorded missed bit and authored grace period. |
| Snapshot text bounds | **CLOSED** | The code-point and aggregate-byte limits remain covered by paged snapshot tests. |
| SDK and guide | **PARTLY** | Both packages build with the pinned SDK. The draft author walkthrough is updated. Installable guide updates and real-host parity remain. |
| Conformance tool crash | **CLOSED** | Conformance tool 1.1.0 accepts both packages at the 14.33 pin. |
| People-thread custody and participant refs | **OPEN** | Production custody and atomic grants for checked participants still need a contract. |
| People-message edits and tombstones | **PARTLY** | Typed saves and message anchors work. Prior history, search and offline deletion propagation remain open. |
| Private per-person thread state | **OPEN** | Account-private per-item persistence, compare-and-set and retention remain fixture-only. |
| Scope and consent for called-in agents | **PARTLY** | Checked disclosure is required. Trusted selection, proof storage and revocation remain open. |
| Called-in agent data and output | **PARTLY** | The per-item denial and import downgrade are defined. Trusted per-author projection is still fixture-only. |
| Placement for a private people conversation | **PARTLY** | The inviter's machine and login are specified by policy. The checked exchange remains open. |
| Kind-wide denial of discovery (14.31) | **CLOSED** | The fixture can still discover the creator's agent thread, excludes the people thread, and excludes the formerly discoverable item after an opaque import commits. |
| Harness egress and OCI acquisition fields (14.31) | **CLOSED** | The manifest declares both fields. Live acquisition and egress enforcement remain host qualification. |
| Generated `ItemSave` and `ItemViewPage` (14.31) | **CLOSED** | Both generated types retain their authored fields and shapes at the new pin. |
| First Send target operand (14.31) | **CLOSED** | Send and alarms carry the target only in top-level `item`. The host injects it into the run input and keeps the contribution input separate. |
| Encoded contribution limits (14.31) | **CLOSED** | Both complete frames are checked with LF before admission and rechecked at launch. |
| Participant identity (14.31) | **CLOSED** | Threads stores `{package,contribution}` and requires a fresh live harness ref for execution. |
| Preflight for encoded contribution size (14.32) | **CLOSED** | The fake refused oversized Send and alarm inputs with `413 contribution_frame_limit`. Neither request left a receipt, pending job or alarm, prompt, revision change, snapshot change or provider call. The next eligible Send admitted once. A later changed contribution input was refused under both a replayed and a fresh call receipt key. |
| Provenance for privacy downgrades (14.32) | **CLOSED** | A staged replacement over the inline limit left discovery intact during stage/seal. Commit set `consent_only` without an explicit operand, preserved the exact imported bytes, and removed the item from subsequent creator discovery. The worker's attempt to clear the marker failed without changing the head. |

### Finding from the 14.33 rerun

| Status and reference | What the plugin needed | Smallest change |
| --- | --- | --- |
| **OPEN** · Generic contribution input types, pinned SDK generator and `ExecutionJobStart`, `ExecutionJobContext`, `AlarmSet` | The wire schemas let the selected interface define the contribution's JSON payload. The generator emits an object-only type for each description-only `contribution_input` property, and its alarm type omits the runtime requirement that an execution alarm with a harness also has an item and contribution input. On dev-01 the wire validator accepted an array contribution payload and refused an execution alarm missing the input. Threads uses an object and sends all three. | Preserve arbitrary JSON for description-only subschemas, and generate the execution/non-execution alarm arms or enforce the dependency in a typed alarm builder. |

### 14.33 host qualification

The fake models checked app-origin writes and an unproven staged import
in one process. The real writer still needs evidence that it can attest
trusted sole-creator continuation, downgrade opaque imports atomically,
and fence in-flight reads, search counts and derivatives from earlier
revisions. The staged test covers discovery before and after publication
and the attempted worker clear. It does not prove live ref invalidation
or a host recertification path.


### Revision 14.34 disposition

| Finding | Status | Evidence or remaining gap |
| --- | --- | --- |
| File custody | **CLOSED** | The Threads app writes the conversation and the harness owns its separate log. |
| Harness ingress and egress | **CLOSED** | Admission pins input and the checked contribution returns bounded output. |
| Harness entry and packaging | **CLOSED** | Both packages pass the ABI, asset and execution policy checks. Placement remains fixture-only. |
| Contribution target identity | **CLOSED** | The qualified app and contribution interfaces match. |
| Item create, save, range and placement | **CLOSED** | Typed inline and staged operations pass. Exact begin and final-save replay return the committed receipt after stage retirement. Changed operands conflict. |
| Rename and lifecycle projection | **PARTLY** | The atomic body, title and leaf and archived predicate are defined. A folder move on rename remains open. |
| Worker context and receipts | **PARTLY** | The pinned input and author context pass; §3 staged replay identities are now specified and exercised in the fixture. Sealed dispatch and durable receipt recovery still need host tests. |
| Create outcome | **CLOSED** | Checked creation still maps to the activation result. |
| Stable identifiers and message links | **CLOSED** | Stable message anchors, portable locators and logical agent identities pass. |
| Alarm lateness | **CLOSED** | The worker consumes the missed bit and applies the authored grace period. |
| Snapshot text bounds | **CLOSED** | Page tests cover complete messages, code points and aggregate bytes. |
| SDK and guide | **PARTLY** | Both packages build with the pinned SDK. The installable guide update and host parity remain open. |
| Conformance tool crash | **CLOSED** | Tool 1.1.0 accepts both packages at 14.34. |
| People-thread custody and participant refs | **PARTLY** | §3 pins one starter-space item and checked full item grants, including item sharing. Checked lookup and atomic creation and grant wires remain open. |
| People-message edits and tombstones | **PARTLY** | Current-head saves and links work. History, search, cache and offline propagation remain open. |
| Private per-person thread state | **PARTLY** | `attempt_disclosure.private_state` defines actor/package/item scope and live access. Compare-and-set, retention and wire shapes remain open. |
| Scope and consent for called-in agents | **PARTLY** | `attempt_disclosure` pins selection, both consent modes, revocation and fences. Trusted authorship, selection evidence and grant wires remain open. The fixture withholds even the caller's old messages without a grant. |
| Called-in agent data and output | **PARTLY** | The policy limits tools to the checked selection and fences delivery and derived output. Per-author projection and tools-on isolation still need live qualification. |
| Placement for a private people conversation | **PARTLY** | Selected-material calls use the caller's enrolled machine and login in a personal space or the configured binding in a shared space. The checked placement exchange and qualification remain open. |
| Kind-wide denial of discovery (14.31) | **CLOSED** | The decision is still per item. A checked fresh creator item is discoverable by the creator's attempts. Ordinary opaque continuation loses that eligibility under 14.34. See the new finding below. |
| Harness egress and OCI acquisition fields (14.31) | **CLOSED** | The fields pass package checks; registry and egress enforcement need live qualification. |
| Generated `ItemSave` and `ItemViewPage` (14.31) | **CLOSED** | The generated fields used by both packages still compile. |
| First Send target operand (14.31) | **CLOSED** | Send and alarms use top-level `item`, and the host keeps the contributed input separate. |
| Encoded contribution limits (14.31) | **CLOSED** | Both LF-terminated frames are bounded before admission and at dispatch. |
| Participant identity (14.31) | **CLOSED** | The stored logical identity remains separate from the checked harness ref. |
| Preflight for encoded contribution size (14.32) | **CLOSED** | Oversized Send and alarms return 413 before any prompt, receipt, revision, provider call or job is saved. |
| Provenance for privacy downgrades (14.32) | **CLOSED** | Staged import sets `consent_only` at publication and the worker cannot clear it. Unproven continuation now has the same rule. |
| Generic contribution input types and alarm dependency (14.33) | **OPEN** | The generator and the relevant wire definitions are unchanged. The object-only type and missing conditional alarm requirement remain. |
| Viewer-scoped row fields | **PARTLY** | The generic private-record policy is defined. Viewer-specific counts, invalidation and their UI binding still need a checked surface. |
| Receipt, typing and device routing | **OPEN** | Muted-chat delivery, the account preference and iOS device state still need host exchanges. |
| Read-only v1 cutover reader | **OPEN** | §3 pins the format identifier scheme. The existing v1 reader behavior is listed below and still needs implementation at cutover. |
| Agent consultation | **PARTLY** | §3 and `attempt_disclosure.consultation` specify a linked item, child-execution ceilings and an independent disclosure check. Live child admission and result authority are unqualified. |
| Account picker and atomic named grants | **PARTLY** | `group_people_scope` pins one checked common group. The fixture exercises mixed-group refusal and revalidation at write. Package-facing exchange and live bootstrap remain open. |
| Account invites and contact identity | **PARTLY** | Group members may issue scoped invites under 14.34. Contact association and package-facing invitation, redemption and default-space bootstrap remain for Contacts. |
| Existing sharing rights and group removal | **PARTLY** | 14.34 grants editing and sharing rights within the item. The owner settled the read-only rule below. The named-grant exchange and live downgrade still need host qualification. |
| Account lookup versus agent discovery | **CLOSED** | `group_people_scope` and `attempt_disclosure` explicitly govern separate checks. Future Contacts policy and live host paths remain open. |

#### Findings added at 14.34

| Status and reference | What the app needed | Smallest change |
| --- | --- | --- |
| **OPEN** · §3 and catalogue `execution_attempt_authority.discovery.item_policy.write` | The checked initial create can set `creator_only`. An ordinary worker body save or append has no generic way to prove sole-creator continuation. In the fixture a personal thread disappears from the creator's agent discovery after the first Send writes its prompt and reply. The job still completes. A rename that rewrites the Markdown likewise has no checked continuation. | Specify a generic checked path that binds exact authored and authorized derived material to the current revision and the creator's authority, or state which personal thread writes intentionally end discovery. Keep the fail-closed behavior until the checked path is enabled. |
| **PARTLY** · §3 and catalogue `group_people_scope.lookup`, `group_people_scope.writes` | The old administration directory can show all accounts to `people.manage` and its ordinary visibility includes any shared group. The Threads picker may use only one checked common group. The old invite capability and the new own-group member rule also differ. | Keep management visibility separate. Pin the checked group selector and a group-scoped invite grant that uses the existing single-use redemption flow. Retain the checked, idempotent default-space bootstrap. |

### Revision 14.35 disposition

| Finding | Status | Evidence or remaining gap |
| --- | --- | --- |
| Personal discovery after normal writes (14.34) | **CLOSED** | Catalogue `item_policy.authorship` supplies the checked paths. Three Sends preserve creator-only discovery after each prompt and reply. The first prompt's owning worker save runs under the checked job invocation, and replies use the exact live attempt. The transcript still attributes replies to the agent. |
| Kind-wide denial of discovery and import downgrade | **CLOSED** | Another person's agent cannot discover the personal item. A people thread remains excluded for both participants. A staged import into the proven personal item still becomes `consent_only` only when published. The worker still cannot clear the marker. |
| Shared write attribution | **CLOSED** for discovery | One participant submits a message using the starter's display name. The fixture binds the other participant's account to the committed revision and the transcript keeps that participant's stable ID. This item-level attribution supplies no trusted per-message consent proof. |
| Group separation and shared custody | **PARTLY** | `group_people_scope.loss_of_common_group` now pins the read-only transition and grant-change fence. The fixture preserves both readers' exports and blocks pending replies. Live lookup, named grants, write and disclosure fences, and default-space bootstrap still need a host. |
| SDK and guide | **PARTLY** | The installable guide and reading-list reference now use the replayable alarm job and `alarm_set`. Both Threads packages build at the pin. Live parity and the generated contribution/alarm type gap remain. |
| Guide for one-date alarms | **CLOSED** | The guide now identifies the checked reminder item and the one due time. |
| Scope and consent, called-in data and output, placement, actor-private state, viewer-specific row fields, consultations | **PARTLY** | 14.35 distinguishes checked item-write attribution from per-message authorship. The previous trusted selection, grant, preference, placement, tools and delivery exchanges remain open. |
| Rename and lifecycle, worker context and receipts, message tombstones, picker/grants, invites/contact identity | **PARTLY** | Each retains its 14.34 disposition. Live write attribution and read-only group enforcement add qualification cases. |
| Generated contribution input and alarm dependency, receipt/typing/device routing, read-only v1 cutover reader | **OPEN** | No relevant wire or implementation change at this pin. |
| File custody, harness ingress/egress and packaging, contribution target identity, item create/save/range/placement, create outcome, stable identifiers and message links, alarm lateness, snapshot bounds, verifier crash, harness egress/OCI, generated `ItemSave`/`ItemViewPage`, top-level Send target, encoded contribution limits, 413 preflight, logical participant identity, separate account and agent discovery | **CLOSED** | The previous dispositions hold. The package and fixture rerun covers the current schemas. |

**New finding, OPEN.** The catalogue says an import, whole-file paste or
hand edit is unproven even when submitted by the creator.
`ItemSave` and `ItemAppend` expose opaque bytes and an optional
`consent_only` request. The fixture knows its staged input came from a
separate import entry. If an import enters through the same checked
owning-package invocation as an authored write, the catalogue does not
pin how the host distinguishes the two. Pin the generic checked source
of that classification, including imported content assembled by a
worker, and test that omitting the marker cannot preserve discovery.

### Messaging decisions fixture

The edit window uses `author_context.authored_at` and the recorded
original posting time. An edit one millisecond before 48 hours succeeds.
A request at the deadline is refused. Deletion preserves the anchor and
author on disk, removes the text, and omits the entire message from the
current native page. A deleted anchor resolves as unavailable. Live
people-thread pages must use the current head, so a superseded page
cannot return an earlier copy of the message in the worker fixture.

An existing reply remains visible after its target is deleted. The file
keeps its `replyTo` for record integrity and message lookup omits that
link from the visible reply. A new reply to a deleted anchor is refused.
Unread counts exclude self-authored and deleted messages. Mentions and
replies count only unread messages addressed to the viewer. A single
message that qualifies both ways contributes one to the row attention
count. A muted chat returns neither a ring nor a push decision. Counts
remain available while muted.

Receipts and typing share one account-level switch, enabled by default.
The fixture disables push until configured and enables both iOS state
options when push is enabled without a narrower choice. Account-level
preferences are separate from each person's per-thread read position,
draft and mute. The fixture covers preference storage and delivery
decisions. Emitting receipts, typing signals, and device notifications
still requires the host.

| Reference | Remaining host or contract need |
| --- | --- |
| API §4.2 retained views and offline copies | Apply a deletion fence to existing results, caches, search and ordinary history delivery. The plugin omits tombstones at the current head and rejects superseded live people pages. Immediate clearing of already-rendered or downloaded content needs host policy. |
| API §§3 and 4.2 viewer-scoped fields | Bind private read position, unread/mention/reply counts and the mute decision to the viewer. Pin the row surface and invalidation. The Claude UI lane owns visual placement and styling. |
| API §3 notifications and owner messaging decisions | Pin delivery-time mute checks, the combined receipt/typing preference, and checked device routing for iOS lock state. The fixture makes decisions with supplied device state and sends no notifications. |

### Conversation and private state on disk

The files below are the exact bytes the package worker wrote through
the fake host on dev-01. Alice and Bob are synthetic accounts. The
called-in Claude Code harness uses scripted output and owns a separate
log. The consultation item shows the private exchange whose summary was
posted back once. The private path is the fixture's account namespace.

The format uses plain frontmatter and one participant per line. Aliases
such as `p1` resolve to stable IDs in the list. Names on its entries are
captured when participants join. Each message heading preserves the
audience-checked name and UTC time from when it was posted. New messages
reflect a later checked display name without rewriting the earlier
headings or changing the participant ID. The agent label is derived from
the contribution's local name until the host supplies an approved
presentation label. Contribution identity stays in the frontmatter.
Names are for display. The host still determines authority.

A message ends at the next unquoted `###` heading with a `{#m-…}` anchor.
Edit body text on the `>` lines, or add ordinary unquoted text or
Markdown between headings. Quote any line that itself looks like a
message delimiter. There is no length to update. The worker reads
hand-edited body text under the same anchor. Such a file edit is an
opaque import for the host's provenance rules and cannot impersonate an
in-app edit or acquire consent. Deletion leaves only the heading on
disk. Conversation views omit it.

`Threads/2026-11-20 Community Garden/Conversation.md`

````markdown
---
format: calternal-thread/v2
id: e5119471e0af708b988694a46ffdd4a9
title: Community Garden
created: 2026-11-20 09:00 UTC
conversation: people
participants:
  - p1 | Alice | person | 019a0000-0000-7000-8000-000000000001
  - p2 | Bob | person | 019a0000-0000-7000-8000-000000000002
  - p3 | Claude Code | agent | a2816f2fd78322bbae0b8609cf0ed511
agent-p3-package: calternal-threads-claude-code
agent-p3-contribution: claude-code
---

### Alice · 2026-11-20 09:01 UTC · edited {#m-b31395e111a687c4 by=p1}

> Can we meet by the garden gate at 10:30?

### Bob · 2026-11-20 09:02 UTC · reply to Alice {#m-eaa58816884f2492 by=p2 re=m-b31395e111a687c4 to=p1}

> Yes, I’ll bring the key.

### Bob · 2026-11-20 09:03 UTC · deleted {#m-264b5e3f491fd782 by=p2}

### Claude Code · 2026-11-20 09:06 UTC · asked by Alice {#m-4daee5fd2fb71be4 by=p3 ask=p1 log=019a0000-0000-7000-8000-000000000010/item-14}

> Reply to the permitted context

### Claude Code · 2026-11-20 09:07 UTC · asked by Alice {#m-9d1c7cac655cb70d by=p3 ask=p1 consult=019a0000-0000-7000-8000-000000000010/item-19}

> Agreed recommendation
````

`.calternal-private/019a0000-0000-7000-8000-000000000001/threads/item-1.md`

````markdown
---
read-through: "b31395e111a687c4"
draft: "I’ll bring the gloves."
muted: true
---
````

`.calternal/thread-consultations/Consultation.md`

````markdown
---
title: Agent consultation
parent: 019a0000-0000-7000-8000-000000000010/item-1
lead: calternal-threads-claude-code/claude-code
consultant: fixture-codex/codex
---

## lead

Private exchange 1

## consultant

Private exchange 2

## summary

Agreed recommendation
````

## Needs host

- Generic installation and cross-package extension resolution, with
  pinned owner app and contributor builds, checked harness and placement
  choices, protected Send, and a verified harness image and entry.
- The typed execution admission, invocation metadata, bounded
  contribution handoff and attempt-fenced output to both owners. Exercise
  successor revalidation and retirement, exact Stop and archive, plus
  failure and restart under the real writer and outbox.
- Generic owned-item create, save, bounded pinned range, append and
  readable placement, plus checked rename, archive and message-link
  navigation. Add staged save before allowing full-document edits
  beyond the inline limit.
- Writer-bound snapshot context/publish and reader-only refresh, prior
  page cursors, follow catch-up and immediate invalidation on revoke.
  Qualify the real create outcome to item route transition.
- One-shot alarm set and delivery with one durable admission, replay,
  cancellation or disable fencing and late timing. Exercise default
  placement in shared spaces and rejection of stale harness refs.

- For people threads, checked participant admission, account-private
  item state, and revision-bound consent and invited-attempt delivery.
  Apply the approved message ceiling to discovery as well as direct reads.
  Define tombstone invalidation and message-anchor routing.

## People-thread fixture

Five people-thread tests passed on dev-01 at 14.30 and 14.31.
14.32 adds the per-item discovery regression.
The emitted worker bundle exposes fixture entry points outside the manifest
and template. They create a people-only conversation in the existing kind,
keep message IDs through edits and tombstones, and store per-person state
as Markdown in a separate fixture record. That record uses compare-and-set
and keeps the furthest read anchor. The fake uses an in-memory broker and
a provisioned set of checked people.

The starter's space holds the single conversation file. The fixture
creates it together with full file grants for the checked people.
They share one item and revision history. Message-level author rules
still govern edits and deletion. The fixture does not implement group
discovery or cross-space transport.

An invitation captures one message, an inclusive range, or the whole
conversation as of that call. Unrelated later changes do not expire it.
Edited or deleted selected text is withheld. A person who does not answer
contributes no messages, and the call proceeds with whatever remains,
including a prompt-only call. Ask-each-time grants in the fixture last
five minutes and apply to one call. Always grants apply to that author's
messages in later explicitly addressed calls in the same conversation.
Either can be revoked through the fixture's consent generation. Revoking
while an attempt is active blocks its scoped read tool and suppresses
its reply. Tools are enabled in the fixture, and the message read tool
can only resolve messages from that call's approved selection. Subsequent
messages need a new call.

A consultation keeps the agent-to-agent exchange in a separate linked
item. The fixture bounds it at three pairs of exchanges and one lead
summary, with up to 4 KiB per turn. The two agents receive the person's
explicit consultation prompt and the private exchange. They receive no
parent transcript by default. The parent gets a single summary with a
portable link to the consultation item. The link confers no independent
read grant.

### Cutover finding for the v1 reader

The plugin writes `calternal-thread/v2` and declares file-format
version 2. At cutover it also needs a bounded, read-only
`calternal-thread/v1` reader following the existing writer and reader in
`crates/server/src/threads/document.rs` and
`crates/server/src/threads/records.rs`. That reader and migration are
outside this fixture change.

- Parse the v1 UTF-8 envelope, materialized `calternal-id`, JSON-escaped
  title, `date`, `last edited`, archive fact and generated title heading.
  Apply the existing rules for stripping the provider notice and private
  session marker. Provider session data stays outside the public view.
- Honor the v1 record's exact UTF-8 **byte** count, framing separators and
  duplicate-ID checks. Authored Markdown can contain lookalike headings
  and comments. Preserve message UUIDs, person/agent/automation role,
  historical heading and offset-aware time, optional agent turn UUID and
  provenance such as provider/model, machine and durations.
- Resolve old deep links using the original item and message UUIDs.
  The 16-hex v2 anchors cannot be used to infer those UUIDs. Preserve
  existing source authority, and default ambiguous authorship to
  consent-only for discovery. Exercise the existing examples, malformed
  records, historical links, and private session suppression before
  enabling the reader at cutover.

### Host needs from the afternoon decisions

| Area | Remaining work |
| --- | --- |
| Shared custody | Atomically create the file in the starter's space and establish full grants for the checked participants. Keep author rules for edits and deletion independent of host privileges. |
| Call selection and consent | Persist the per-conversation preference and call-bound selections. Fence delivery, derived outputs and writes on revoke, expiry or delete. Qualify active attempts and both modes against a real host. |
| Tools | Enable the called-in agent's tools under the checked call. Enforce the selected-content ceiling across every read surface and prevent broader search or follow-on reads. The fixture demonstrates only a scoped read tool. |
| Consultation | Authorize both placements independently, bound the exchange, persist replay and cancellation, and publish just the final linked result. Require separate authority before sharing parent transcript content with a second agent. |

### Contacts and sharing findings

The Threads app follows the existing account model. The existing
management directory grants broader visibility through `people.manage`.
14.34 keeps that protected directory separate and limits the Threads
picker to the checked common group. The same fixture predicate checks
the picker, selected-account lookup and commit of named grants. A person
may appear through a local or mirrored group. The fixture rejects hidden
and unknown IDs with the same error, including on replay.
[ADR 0058][adr0058] · [ADR 0060][adr0060] · [Existing share commit][share-commit]

In the existing app, account invites are single-use onboarding links issued
under `people.invite`. 14.34 allows members to issue links scoped to
their own group and calls for a default shared space per group. The
package exchange stays open. Redemption creates the account and applies any assigned
local memberships together. An account becomes eligible to pick when the
same directory rule can see it. The picker deals in account IDs. Contact
cards and CardDAV lists have their own identities and can supply contact
information through an explicitly verified account association if
Contacts adds one. [ADR 0059][adr0059] · [ADR 0063][adr0063] · [ADR 0064][adr0064]

| Finding | Smallest contract change or decision |
| --- | --- |
| API §§1, 3, 5 and ADRs 0058, 0060. The catalogue has no checked account selector or atomic named-share creation for community plugins. A `people.invite` or `groups.manage` capability alone grants no broader selector visibility. | Provide a host-owned picker source and checked named-share creation driven by the same live visibility predicate. Recheck access before returning saved receipts and when committing grants. Return stable account IDs and display names from the checked group and keep private group and account details in the directory. |
| ADRs 0059, 0063, 0064 and the Contacts handoff. An account invite grants onboarding and group membership. The existing contact file ID and imported card UID identify contact data. Neither supplies a named share for an unregistered recipient. | Keep onboarding in the host. If Contacts later supports an invite from the picker, establish the account through the existing invite flow and check current visibility before sharing. Leave card-to-account association with Contacts. |
| Afternoon full participant privileges and the [existing sharing decisions][sharing]. One starter-owned file agrees with the existing owner's-file and named-share model. Existing named grants expose read or edit and leave share management with the owner. 14.34 explicitly includes changing shares within the item. | Pin the checked exchange and map whole-item lifecycle operations such as deletion to existing capabilities. The owner's follow-up keeps named read and export access when common group membership ends and removes item write and agent invocation authority for all participants. |
| ADR 0060 and the afternoon discovery rules govern different checks. Local and mirrored groups control whom people can find and select. Attempt discovery is per item and remains consent-scoped for conversations between people. | Keep group membership as an input to account selection. Keep the agent's selected-content and author-consent check on its separate admission path. Leave live group audiences and further discovery rules to Contacts. |

[adr0058]: https://git.kayg.org/kayg/calternal/src/commit/ea57b84b9c9e40e8b67b7d1e1f1805915123b040/docs/adr/0058-roles-are-capability-sets.md
[adr0059]: https://git.kayg.org/kayg/calternal/src/commit/ea57b84b9c9e40e8b67b7d1e1f1805915123b040/docs/adr/0059-invites-and-storage-quota.md
[adr0060]: https://git.kayg.org/kayg/calternal/src/commit/ea57b84b9c9e40e8b67b7d1e1f1805915123b040/docs/adr/0060-groups-and-person-visibility.md
[adr0063]: https://git.kayg.org/kayg/calternal/src/commit/ea57b84b9c9e40e8b67b7d1e1f1805915123b040/docs/adr/0063-contact-notes-keep-original-vcard-lines.md
[adr0064]: https://git.kayg.org/kayg/calternal/src/commit/ea57b84b9c9e40e8b67b7d1e1f1805915123b040/docs/adr/0064-contacts-sync-over-carddav.md
[share-commit]: https://git.kayg.org/kayg/calternal/src/commit/ea57b84b9c9e40e8b67b7d1e1f1805915123b040/crates/auth/src/sharing.rs#L2546
[sharing]: https://git.kayg.org/kayg/calternal/src/commit/ea57b84b9c9e40e8b67b7d1e1f1805915123b040/docs/grills/sharing.md

#### Group separation

The owner decided on November 20 that when the people in a conversation
no longer share a group, they keep reading and can export its history.
The Threads app refuses posting, editing, deletion, rename, invitations
and consultations. Both readers can still obtain the exact Markdown
through the checked range path. The fixture derives writability from a
currently common group across all named people, using the host's grants
and live directory. Private read position remains writable. The tests
also exercise loss of membership during a called-in agent's run, fencing
further tool reads and the pending reply.

The host still needs a checked conversation-write surface and must
apply the downgrade under its authority fence at admission, tool
delivery and commit, including host-driven archive and shares. It must
preserve named read and export access to the same item when it fences
writes. The fixture models those permissions with the in-memory directory.

## Scope and limits

Live Claude Code execution and result delivery today would require the
legacy native provider path or new public execution and contribution
APIs. Directly appending from the harness to the thread would also
require a cross-owner write grant that the design does not provide.

The package worker code uses no native Threads dispatch, special plugin ID,
direct package connection, inherited provider credential, or server-owned
thread format. The fixture provider port stands in for live execution placement. The
14.35 fake carries replies through the catalogue's checked contribution
methods. People-thread invitations still use fixture ports. The harness manifest includes
an image-shaped fixture digest and a pinned entry that fails closed.
It cannot launch Claude Code. The real image, login mapping and reviewed
placement require the same public installation path as any contribution.
The worker bundles export draft handlers. The fake adapts validated
invokes through the pinned SDK author-context adapter. The sealed dev
stdio runtime remains separate.

The fake models checked items and revisions in memory and scripts a single
reply per run. It supplies a stable request identity, actor and time as
fixture data. Rename retains the original readable directory, and a
same-title path obtains a numeric suffix in the fake. Titles use a
120-byte bound and message bodies 16 KiB. One document is bounded at 2 MiB
and 64 participants. Inline saves stop at 128 KiB. Moving from the fixture
to durable storage needs scoped receipt retention and bounded history,
and the per-space snapshot and alarm caps already in the catalogue. The
spike has no format migration, attachment upload, historical attribution,
continuation or provider tools. The people-thread operations are fixture-only.

Send and alarm setup each read the current document to check whether
an invitation is required. With a launched run, the worker reads one
revision-pinned document and writes the prompt and reply once each. The
first run or a new participant may use a full save for frontmatter; established participants use append.
A head probe after the harness returns detects intervening edits and only
then reloads. Each successful content head publication takes one context
call and one publish call. A page read does one bounded document read
with 64 KiB range calls and publishes nothing. The fixture does not
perform storage flushes. A conflict at the final append fence may need
durable job replay. The placement and output contracts must say how a
verified result survives that replay without invoking the provider again.

## Claude UI handoff

Design the Threads app list and its active/archived states, transcript and
per-message link affordance, composer with harness and placement choices,
queued-follow-up and Stop controls, and the automation date and missed-run
settings. Check cold-to-live transcript replacement and attribution in
the native renderers. The spike template is a form-level operation
fixture. The real UI should use the host confirmation and execution
controls for sensitive actions.

Use the host account selector for the people picker when Contacts supplies
it. Submit named account selections and handle an unavailable selection
when visibility changes before confirmation. [ADR 0060][adr0060]

## Owner questions

- **Should a missed run mean the server was unavailable or simply that it
  started late?** This matters before someone can rely on Skip. A 9:00
  alarm can reach a healthy worker at 9:00:01, or wait until 10:30 after
  an outage. Treating any delay as a miss is simple and skips healthy
  runs. An outage-only marker preserves those runs but misses long
  healthy queues. A configurable grace period covers both at the cost
  of an extra schedule setting. Recommend a defined grace period, with
  one default for the initial UI and both timestamps shown in history.
- **Should renaming a thread move its folder?** The file layout needs
  this decision before old paths become export references. Renaming
  “Lease” to “House paperwork” could move the directory, giving a
  readable export but invalidating bookmarks to the old path. Keeping
  the original folder avoids moves and keeps old file references
  usable, with an old folder name beside a new title. Recommend the
  original folder for the first release and message links based on
  stable item/message IDs.

## Simplification opportunities

- Use the same checked Send operation for initial admission and the
  successor. Keep queue capacity, promotion and retirement in the
  generic job engine.
- Use the same native page generator for retained and live reads, with
  writer and reader authority enforced at their separate entry points.
- Put shared broker request and result shapes in the catalogue once and
  derive worker typings, fake validation and author examples from them.
  That removes the spike's fixture declarations and draft packaging script.
