# Threads plugin spike · 2026-11-20

The lane starts from dev `80864d0d` and pins Plugin API v2 draft 14.30 at
`b89586dd306784e94ca09992ae05180d7bb7a686`. It follows the November 20
decision that Threads owns the conversation file and the selected harness
owns its working log. The lane issue is [#92](https://git.kayg.org/kayg/calternal-ai/issues/92).

Threads creates one Markdown item per conversation, in a readable dated
folder. Its frontmatter holds the stable conversation ID and people and
agent participants. Length-framed Markdown message records hold stable
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

Run on dev-01 in the lane directory with the draft commit available
locally and the dependency cache populated.

```sh
export TMPDIR="$HOME/gate-tmp"
nix develop -c npm ci --offline --ignore-scripts --no-audit
nix develop -c npm run build -w @calternal/plugin-sdk
nix develop -c npm test -w @calternal/threads
nix develop -c npm run conformance -w @calternal/threads
```

The offline build emits both manifests, pinned worker bundles, the minimal
Threads template and the harness fixture entry. The tests load the emitted
bundles and manifests. The build checks schema shapes and asset hashes.
Tests check recorded, typed exchanges using the draft's schemas. The
unmodified conformance command runs the pinned tool separately and fails
on both packages at `conversation_capability`. Full conformance and live
provider placement remain blocked.

Nine tests on dev-01 exercise create, first Send with the contributed harness,
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
| Item create, save, range and placement, §3 engine surface and staged save | Common item methods have prose but no catalogue request/response schemas. The create needs a checked item ref, a revision, and a readable dynamic child folder with collision behavior. The kind's folder is static. Staged save describes begin/chunk/seal but the author method enum has no precise staged calls. | Add typed create, save, head and range, including safe relative path components under the kind's declared root, atomic collision handling, a typed head conflict, and typed staged save arms. Use the existing content writer. |
| Rename and lifecycle projection, API §§3 and 4.4 and catalogue `host.item.rename` / `host.item.archive` | Rename of the app title can update the Markdown header with the indexed metadata, or generic Rename can update placement and Undo. No declared operation can combine both. Archive is generic and fenced, but the generic field table does not expose an `archived` list predicate. | Add a checked owner-authored body and title/placement update in one content receipt if folders should follow titles. Expose the generic archive fact to filtered Binds without asking owners to mirror it. |
| Worker context and receipts, §§3, 4.3 and 7, SDK `worker-v2.ts` | Worker code needs the live job and alarm metadata, stable parent request identity for subeffect IDs, stable actor attribution and an authored timestamp that survives replay. The declared host frame includes job and alarm, but no author-facing request, actor or stable clock context. The dev SDK worker accepts a v1 manifest, v1 host-call inventory and a separate fixture frame with no revision 14 context. | Generate a typed author context from the revision 14 carrier, exposing only stable author data and protected job context. Derive subeffect receipts from that context. Teach the SDK how to encode refusal status and retryable head conflict. |
| Create outcome, API §4.3 and manifest `operation.creates` | A worker-owned create must become an activation `created` ref for navigation. The generic outcome has the field and the manifest declares a kind, but their mapping isn't specified for worker results. The spike uses a `created` output. | Pin the output convention and require the host to check that the exact invocation produced the returned same-kind ref. |
| Stable identifiers and message links, API §§3 and 4.2, catalogue `view_snapshots` | The app can store a stable message ID and look it up in a checked item. `ItemViewRead` only accepts latest or older. Snapshot leaves have no message anchor and `ItemRef` / `HarnessRef` specify checked scope, leaving portable, durable identity for the log link and participant uncertain after reinstall, export or receipt expiry. | Define an authorized item locator and optional opaque message anchor on item-view open and leaves. Specify which IDs can be stored as portable references versus only used as live operands. |
| Alarm lateness, API §3 and `AlarmDelivery` | Timestamps distinguish an exact on-time admission from a delayed one. They cannot tell an outage miss from ordinary scheduler jitter, so the spike's strict comparison would skip normal late-by-milliseconds runs. | Define scheduler delay semantics, or add a host-attested missed-window fact while leaving the action choice with the plugin. |
| Snapshot text bounds, description `SnapshotText` and API §7 | JSON Schema `maxLength` counts characters. The pinned validator rejects a string of at most 2,000 multibyte characters when its UTF-8 encoding exceeds 2,000 bytes. The spike splits at 2,000 bytes and keeps a whole message together in its page. | Specify the intended unit and add a Unicode boundary fixture shared by SDK, tool and host. Keep the independent aggregate byte cap. |
| SDK and guide, API §7 | The dev guide and offline builder target the sealed contract. The author and renderer generators can consume draft schemas, but the sealed SDK build rejects draft manifests and packages only the older verifier module list. The public guide does not show execution jobs, alarm evidence, view publication or a cross-package harness. | Ship a revision-aware validator, package builder and broker context, and a walkthrough that runs the same plain app plus separate harness example on the real and fake hosts. |
| Conformance tool 1.0.0, `description_verification.py` | `check_manifest_install` unconditionally reads the catalogue's removed `conversation_capability` key. Both package runs crash after local manifest shape checks. The same module's worker-invocation check still keys on `execution_intent` while the draft job declares `execution`. | Update the tool to the revision 14 catalogue and add regressions for a plain `execution_target` app manifest, a separate contribution manifest and an alarm execution job invoke. |

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

## Scope and limits

The package worker code uses no native Threads dispatch, special plugin ID,
direct package connection, inherited provider credential, or server-owned
thread format. The fixture ports for provider invocation and reply
delivery stand in for missing public APIs. The harness manifest includes
an image-shaped fixture digest and a pinned entry that fails closed.
It cannot launch Claude Code. The real image, login mapping and reviewed
placement require the same public installation path as any contribution.
The worker bundles export draft handlers. Dev's sealed stdio runtime
cannot dispatch them.

The fake models checked items and revisions in memory and scripts a single
reply per run. It supplies a stable request identity, actor and time as
fixture data. Rename retains the original readable directory, and a
same-title path obtains a numeric suffix in the fake. Titles use a
120-byte bound and message bodies 16 KiB. One document is bounded at 2 MiB
and 64 participants. Inline saves stop at 128 KiB. Moving from the fixture
to durable storage needs scoped receipt retention and bounded history,
and the per-space snapshot and alarm caps already in the catalogue. The
spike has no format migration, attachment upload, historical attribution,
continuation, provider tools, or conversations between people.

With a launched run, the worker reads one revision-pinned document and
writes the prompt and reply once each. The first run or a new participant
may use a full save for frontmatter; established participants use append.
A head probe after the harness returns detects intervening edits and only
then reloads. Each successful content head publication takes one context
call and one publish call. A page read does one bounded document read
with 64 KiB range calls and publishes nothing. The fixture does not
perform storage flushes. A conflict at the final append fence may need
durable job replay. The placement and output contracts must say how a
verified result survives that replay without invoking the provider again.

## Claude UI handoff

Design the Threads list and its active/archived states, transcript and
per-message link affordance, composer with harness and placement choices,
queued-follow-up and Stop controls, and the automation date and missed-run
settings. Check cold-to-live transcript replacement and attribution in
the native renderers. The spike template is a form-level operation
fixture. The real UI should use the host confirmation and execution
controls for sensitive actions.

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
