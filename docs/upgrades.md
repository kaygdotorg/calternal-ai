# Data-preserving upgrades

The production schema baseline is dev `c98705d74bcf9f9cb2a1292b849b9b44c64b2714`,
installed fresh as image tag `c98705d74bcf`. The markers below describe that
release. The update proof uses the actual running image ID, which the operator
captures in the [deployment runbook](deploy.md#first-ordinary-update-from-the-production-baseline).
Preserve the earlier steps in the migration registry. Unsupported preview
data is refused.

## Durable inventory

Paths with IDs always use account or space UUIDs. SQLite WAL belongs to its
database until checkpointed. SQLite SHM, locks and sockets are runtime state.

| Root and relative paths | Class | Writer and baseline marker |
| --- | --- | --- |
| Appdata `system/auth.db` | Authority | Auth, SQLite 14. Accounts, passkeys, sessions, roles, memberships, invites, approvals, machine identity and grants, erasures, key fingerprints and protected receipts. Versions 12 and 13 add account recovery and provider sign-in. Version 14 adds dormant sharing grants. Version 15 adds the sharing purge capability grants. |
| Appdata `system/appearance.db` | Mixed | Appearance, new SQLite 1 baseline through the shared runner. Account preferences and original private uploads are authoritative. Shared manifests and eviction pins coordinate with the cache. The production baseline has no appearance database. |
| Appdata `fonts-cache-v1/` | Derived | Version 1 directories of immutable WOFF2 bytes by family and digest. Startup verifies the published manifest and retires incomplete or orphan fills. |
| Appdata `system/machines.db` | Authority | Machine placement, SQLite 6 initial schema, advanced to 7 on server startup by the automatic thread-title lease migration. Version 8 adds scoped harness leases. Version 9 adds shared-space execution bindings and the lease sponsor. Leases, fences, drain, work locations, discard and erase delivery, and the durable stop record that keeps an erased worker's machine slot reserved until its stop completes. |
| Appdata `spaces/<id>/content.db` | Mixed | Content, SQLite 5. Item identity, revisions, pending placement, trash and tombstones, kinds, plugin controls, thread snooze state and sharing's collection membership. Version 6 adds live-room kinds and document checkpoints, version 7 adds Trash jobs, and version 8 adds rename jobs. Listing, labels and links are derived. |
| Appdata `spaces/<id>/index.db`, `secrets/spaces/<id>/head` | Authority | Space journal, SQLite 2 and binary CHW2. Journal, epoch, receipts and independent head witness. Receipts are pruned after the operator's retention (7 days by default) only on content's command, with every receipt of a surviving outbox intent pinned. The fence adds no stored state. |
| Appdata `spaces/<id>/network.db` | Authority | Plugin network, SQLite 1, introduced after the production baseline. Per-person remote content preferences and bounded outbound audit metadata. Absent databases use the default and are initialized by the shared runner on the first saved preference. |
| Appdata `spaces/<id>/jobs.db` | Authority | Jobs, SQLite 2. Admission, retry and in-flight state, checkpoints, event outbox and retention. Version 3 adds plugin signal waits and decision receipts. |
| Appdata `accounts/<id>/input.db`, `content.db`, `agent-settings.db` | Authority | Account input, content and jobs. SQLite 2 for drafts and pending input, 1 for protected collections and 1 for retry settings. Input version 3 adds private plugin settings and their receipts. |
| Appdata `accounts/<id>/onboarding.db` | Authority | Account onboarding, SQLite 1 when a person finishes onboarding. Missing files read as unfinished. The completion marker rides with account settings and is removed by account erasure. |
| Appdata `accounts/<id>/secrets.db`, `spaces/<id>/secrets.db` | Authority | Secrets, SQLite 1. Sealed values, pending prompts, audit and key state. |
| Appdata `accounts/<id>/web-push.db` | Authority | Browser push, SQLite 1. At most eight 90-day subscriptions per account, scoped by installation. |
| Appdata `system/web-push-vapid-v1.der` | Authority | Browser push signing key; bounded P-256 SEC1 DER with a versioned filename. Preserve it with the subscription databases. |
| Appdata `system/invite.key`, account and space `secret.key` | Authority | Auth and secrets. Fixed 32-byte payloads with v1 key derivation and ciphertext binding. Preserve bytes, ownership and mode with their database. |
| Appdata `installed-plugins.sqlite3` and `installed-plugin-workers/` | Mixed | Package store, production SQLite 1. Version 2 adds live editing to the original bundled Notes archive. Version 3 adds reviewed provider runtime metadata and the shipped provider's sign-in registration, preserving installed and retained archives, checksums, activation approvals and revisions. Manifest API version 1 describes the package contract. Extracted worker copies are a cache reused across restarts only when they match the archive bytes, so they carry no version. Development drafts are disposable. |
| Appdata `system/content-index.db`, `spaces/<id>/search-v2.db`, `spaces/<id>/search-messages-v1.all.db`, `wallpapers/` | Derived | Content routing, SQLite 1. Retain the exact route catalogue across upgrades. Item search is SQLite 2 at generation 3. Message search, with its postings and thread-location index, is SQLite 1 at generation 1. Wallpaper cache follows its rendition catalogue. |
| Appdata `system/automations.db` | Authority | SQLite 1, introduced after the production baseline. Scheduler cursor, stop state and run dispatch outbox. The shared SQLite runner owns the schema. |
| SSD space `Notes/`, `AI/Chats/`, Trash, registered visible and hidden content | Authority | Content writes `calternal-id` and a `calternal-note/v1` envelope for Markdown without its own format tag. Thread writers use `calternal-thread/v1`. Automations use `calternal-automation/v1`. Portable skill entry points use the standard Agent Skills format inventoried here and carry standard frontmatter. Space Collections use `calternal-collection/v1` JSON for their structured membership. Other provider or plugin formats and opaque bytes follow their own contracts. |
| SSD space `AI/AGENTS.md`, `AI/Skills/*/SKILL.md` | Authority | Space instructions and the person write raw Markdown. Skill descriptors use the external Agent Skills format and may be provisioned on disk. These formats pass through byte-for-byte. |
| SSD `spaces/<id>/.calternal/settings.json` | Authority | The plugin settings adapter writes `calternal-plugin-settings/v1`. JSON preserves typed nested values, including unknown settings during an older writer’s update. |
| SSD account shortcuts, themes and other settings files | Authority when introduced | Use `calternal-shortcuts/v1` for account shortcuts and `calternal-theme/v1` for themes. Custom components and starred prompts need their own versioned formats when their writers land. |
| SSD `.calternal/recovery/snapshot.json` and `log-*.bin` | Authority | Recovery coordinator writes v3 snapshots and checksummed frames, paired with the auth DB witness. |
| HDD originals and each root's `.calternal-install-id` | Authority | HDD originals are opaque. There is no general HDD placement writer today. Roots writes `calternal-install-root/v1` with installation UUID and role. |
| Machine `credentials`, `pending-enrollment/credentials`, and `machine-handoff/{request.json,enrolled}` | Authority during use | Machine enrollment writes `calternal-machine-credentials/v1`. Both roles use `calternal-machine-enrollment/v1` for the handoff. Protected files stay on the machine. |
| Machine `sandboxes/accounts/`, worker/container state | Mixed | Sandbox, harness and person write per-space homes, transcripts, account provider login and thread workspaces. Provider bytes are opaque. Worker containers, temporary login work and execution state are disposable. MicroVM mode keeps each account's tree in `microvm/accounts/<account>.ext4`; the ext4 label `calternal-v1` versions the disk layout and an unknown label is refused untouched. `microvm/run/` and `vmm.lock` are runtime state. |
| Machine operator files for microVMs | Operator | The guest image is pinned by its manifest digest in the machine config. The machine config rejects unknown fields. The egress policy's `.installed` receipt is bound to the boot and fails closed, so it is reinstalled rather than migrated. |
| Appdata `system/upgrade-intent.json` and role-local `.calternal-upgrade/` | Temporary authority | Deploy writes a private v1 intent for additive updates or a `calternal-upgrade-intent/v2` intent for a full restore point. Both bind the installation, both image digests and payload checksums. Before v2 publication, deploy pins the candidate recovery executable under `releases/pending-recovery`. The committed intent returns to v1 after retiring the full manifest. The gate supplies `calternal-upgrade-proof/v1` evidence for the two images. |
| Appdata `startup.json`, `setup.token`, operator and release config | Mixed | Server startup publishes `calternal-startup/v1` and a replaceable setup code. Auth retains its hash and expiry. Operator owns the pinned installation ID and deployment secrets. Deploy owns image IDs, units and release pointers. |

The live-editing migration adds `room_kinds` and `room_checkpoints` to
space content SQLite as version 6, following Sharing’s version 5 collection
membership projection. Checkpoints belong to an immutable revision and commit with its outbox
intent. A checkpoint is limited to 8 MiB. Each item retains up to 64 recent
checkpoints and 16 MiB total. Purge removes them with the referenced revisions.
The preceding writer ignores these tables. A head written without a
checkpoint is a new lineage on the next live access. Older live bases cannot
be merged automatically across that boundary.

The installed package catalog advances from SQLite version 1 to 2. The
shared runner adds live editing to the original bundled Notes archive
and updates its archive checksum. The migration matches the exact
pre-live archive bytes and checksum and preserves its activation and
revision. It leaves explicitly approved packages and retained versions
as stored. Fresh installs receive the opt-in from the shipped manifest.

## Forward changes

The shared runner in `calternal-store` ports the existing app's
`BEGIN IMMEDIATE` and `PRAGMA user_version` sequence. A current store pays one
version read. A store with pending steps rechecks under the write lock and
commits all steps and their version changes together. Keep the first shipped
schema and every shipped step fixed. A version outside the declared range
fails before schema changes. The deployment gate will admit a newer version
for an older release only under the explicit, tested compatibility policy.

Production image `c98705d74bcf` has auth version 14, content version 5
and machines version 6. After live rooms at content version 6, Empty Trash
and rename queues follow at versions 7 and 8. Auth version 15 adds the
Manager and Owner purge grant.
The prior writer still allows Editors to purge. Treat this rollout as
incompatible and use the owner-approved restore point for rollback.

Message search generation 2 drops the redundant thread-location index and
rebuilds under the projection worker. Its SQLite schema version stays 1.

Owned tags put the name before the version, as in `calternal-thread/v1`.
Binary formats retain the version markers listed above.
An unknown version of a recognized format stays untouched. File readers
report unsupported format. The thread API returns a conflict with
`details.reason = "unsupported_version"`. Format-independent user Markdown,
external skills, opaque originals and provider state keep their original bytes.

Notes, labels and thread-list indexes share a derived generation in each space's
content DB. A mismatch replaces only the index structures and term rows in a
transaction on first space use. Search uses its own generation and catches up
in the bounded worker. Neither rebuild discards revisions, receipts, plugin
controls, pending writes or routing. A future route catalogue rebuild needs a
ready-generation proof before a UUID miss can be treated as absence.

The search WAL ceiling is 64 MiB per projection database. The VFS enforces
it and the writer requests a checkpoint at 48 MiB. Both are runtime
limits. Draft saves use `synchronous=NORMAL` and suppress checkpoint on
connection close. Those connection settings leave the account input format
at SQLite 2. Acknowledged unsent edits survive a process crash. The latest
unsent edits can be lost during a system crash or power loss.

The lease migration adds nullable harness confinement and login-scope metadata; legacy lease rows retain their existing behavior.

### How to add a migration

1. Identify the writer and classify the change. For an authoritative SQLite
   store, find its `calternal_store::migrations::Schema` declaration. The
   writer and the deployment inventory must use that same descriptor. Keep
   the baseline number, initial batches, and previously shipped steps fixed.
2. Append a `(version, SQL batch)` entry to that store's descriptor. The
   runner checks for consecutive versions and writes `PRAGMA user_version`
   in the same immediate transaction as the batch. Leave transaction control
   and version writes to the runner. Include any required deterministic backfill and
   preserve ownership, references, and receipt replay. A new store starts
   with an explicit baseline and an inventory row.
3. Keep the preceding release's reads and writes valid for unattended
   updates. Review its inserts and updates as well as its queries. Classify a
   change that breaks that contract as incompatible and obtain the owner's
   approval for a full restore point. Run the release coordinator for the
   deployment so it can snapshot the changed authority and its witnesses.
4. For a rebuildable projection, bump its generation when its stored layout
   or interpretation changes. A generation is a plain constant beside the
   projection's schema, never the package version, so an unrelated release
   does not rebuild every space. Rebuild from the authoritative records through
   the normal admission path. For an owned file change, update its version
   tag and inventory entry, and specify what a reader does with an unsupported
   version.
5. Test a fresh store, a store populated through the preceding release, and
   the failed-transaction path. Extend the retained fixture when it lacks the
   affected data. For unattended deployment, run the exact image-pair gate
   below and check reads, acknowledged writes, and rollback against that data.
   The test-only `upgrade_schema()` step in `machines/db.rs` shows the shape of
   a real step and how the fixture tests drive it.

**Claiming a version.** Choose the next number in the store's numbered list
on your branch. The number becomes fixed when the migration merges into
`dev`. If another lane has already taken it, keep the merged entry and
renumber your pending step to the next free number when merging. The shared
descriptor rejects duplicate numbers and gaps.

**Protected auth writes.** Use a plain `UPDATE` and, if no row matched, a
plain `INSERT` inside the same protected transaction. The `DO UPDATE`
branch of a SQLite UPSERT can override the dirty-key trigger's
`INSERT OR IGNORE` and fail on an already queued owner key. Keep the
recovery trigger and witness publication in the write path.
[Recovery protocol](../crates/recovery/PROTOCOL.md)

The jobs.db 2 → 3 change preserves existing rows and the original
jobs-table writer shape. A release at version 2 cannot service parked
plugin signal waits. Qualify downgrade semantics with the exact release
pair before activating the feature on production data.

### Item sharing

The fresh production baseline at `c98705d7` includes dormant sharing authority
(`auth.db` 14) and the derived collection membership index (space
`content.db` 5). The sharing feature uses these shipped schemas. The existing
`system/invite.key` backup also protects public link credentials through
independently derived keys.

## Release handoff

Unattended updates allow additive changes with proof that release N can run
against N+1 data. An incompatible upgrade requires the owner's explicit go and
a full restore point. There are no reverse transforms or dual writers.

Stage the candidate image and its compatibility evidence while N serves. Drain writes and machine work
at handoff, lock the roots, check versions, and publish a durable intent and
snapshot of changed authorities with their witnesses. Apply migrations and
verify integrity and private readiness. A durable committed-intent marker
ends the snapshot restore window. The intent is limited to 32 MiB. Its private backup
bytes are retired before serving writes. Failure before that marker restores N and
its data through the deployment rollback path. Afterwards, a compatible N
can resume on live data, including any new acknowledgements. An incompatible
downgrade refuses newer data. Retain one restore point during handoff and one
prior release. The deployment script and the server unit share the local
recovery command. Ordinary additive updates snapshot only changed stores and their witnesses.
Before stopping the roles, the deployment driver checks the exact image proof
and plans the snapshot footprint. It repeats both checks under the storage
locks. A live preflight refusal leaves the current services running. A locked
refusal after the stop restarts the previous release.

A full restore point records directories and regular files across appdata,
SSD, HDD and the co-located machine root. Linux descriptor-relative lookups
refuse nested mounts. Symlinks retain their target bytes, including absolute,
escaping, dangling and cyclic targets. Capture and restore operate on the
link entry without traversing it. Hard links to regular files retain their
inode relationships within a role. Links with other names outside that role
and multiply linked symlinks are refused. Unix sockets and runtime locks are
excluded. Devices, FIFOs and unknown entry types are refused by path.

| Last durable state | Recovery |
| --- | --- |
| Preflight or locked checks | No snapshot or migration is published. Resume the prior release if the roles were stopped. |
| Copying | Remove partial payloads, then resume the prior release. |
| Preparing, including the private candidate probe | Validate the manifest and every payload, replay against the locked roots, remove additions made during preparation, then resume the prior release. |
| Restore interrupted | Replay from the immutable manifest and payloads. A planted directory symlink fences restore. |
| Committed or retirement interrupted | Retire payloads and finish the candidate's activation. Rollback requires a previous release that understands the live data. |

The v2 intent checksum covers the image IDs, installation, phase and
manifest, including link metadata and the payload checksums. Recovery
validates it before making any changes to the captured roots.

The private full-upgrade probe uses the candidate image with isolated
networking and a loopback health endpoint. It holds the normal storage locks
and finishes before the durable decision turns on public writes. A retry,
rollback or startup hook uses the pinned recovery binary while the previous
release is still current.

## Release gate

On dev-01, run `scripts/verify-additive-upgrade.sh PREVIOUS_IMAGE CANDIDATE_IMAGE PROOF_OUTPUT`
before handing an additive release pair to deploy. The script extracts each
image's executable and plugins, starts release N on disposable roots, and
populates them through the authenticated API and operator CLI. N+1 migrates and
starts on those roots. Each release reads a write acknowledged by the other.
The proof file binds those image digests and is published only after the test
passes. Review each change for compatibility with the previous writer as well.

The [retained fixture](../tests/conformance/fixtures/upgrades/README.md) records
the seed revision. Its debug upgrade exercises a forward schema change, nine
crash points, and refusal of a downgrade without additive approval. The
fixture uses disposable data and keys.

The working budget is five seconds of unavailable writes for an ordinary
additive update. On the locked perf-01 VM, ten prebuilt-binary handoffs on the
small fixture measured 164.9–200.0 ms from the last old acknowledgement to the
first new acknowledgement, with a 178.7 ms median. This includes graceful
process stop, snapshot, migration, verification, commit, and process startup.
That measurement covers server process and storage only. The deployment
rehearsal below measures the full write gap through the front proxy.

### Deployment rehearsal on dev-01

Use a unique instance name of the form `calternal-ai-u46-<lowercase suffix>`
and build the two release images as `localhost/$name:baseline-source` and
`localhost/$name:candidate-source`. Generate the proof for those IDs and
extract the baseline machine unit from the baseline source revision. With
workspace npm dependencies installed, run the following on dev-01.

```bash
nix develop -c deploy/rehearse-upgrade.sh "$name" "$dev01_netbird_ip" \
  "$proof" "$baseline_machine_unit"
```

The script creates ten disposable installations. Each run uses the deployment
driver, rootless Podman, user Quadlet and a live enrolled machine service. A
local HTTPS proxy forwards the signed-in browser's requests. The driver uses
a same-VM substitute for its SSH hop. The machine uses a private loopback
listener and starts with no job in flight. The probe records the interval from
the last old-release note write acknowledgement to the first candidate
acknowledgement. The run also checks that a refused proof leaves the original
container and session reachable, and that an untimed rollback retains the
candidate's acknowledged write.

On November 18, 2026, the dev-01 rehearsal used the `c98705d7` baseline
and the `1ab70e7d` upgrades image. The ten gaps in seconds were 95.203,
92.416, 93.954, 92.821, 92.851, 93.872, 94.903, 94.536, 94.334 and 93.926.
The median was 93.940 seconds and zero runs met the five-second budget.
All ten retained the candidate's write across rollback. The first attempt
at run seven passed the refusal check but reached the harness watchdog
before recording a complete handoff. Run seven and the remaining trials
used the same images with a longer harness deadline.

The proxy returned HTTP 502 during the write gaps. Some probes received no
HTTP response. The page stayed signed in and the reconnect banner never
appeared. The test sends its writes through the browser's API context. In
Notes, a failed save can display **Not saved** because the client treats a
non-JSON gateway response as a failed operation.

Podman waited for the configured 90-second stop deadline with the signed-in
tab open, then sent SIGKILL to the old server. The server's graceful
shutdown fences workers and machine leases, but does not end the live
account, space and job event streams. Closing the tab before the untimed
rollback let the server stop promptly.

End those streams on SIGTERM, preserve their resume cursors, and let
in-flight writes finish through their durable acknowledgement fences.
Measure the remaining Podman, machine and proxy handoff with the same
ten-run probe. During the outage, treat proxy 502, 503 and 504 responses as
temporary unavailability so the interface can show the reconnect state and
retry queued operations with their original request IDs.

## Account onboarding and recovery

Auth schema 12 adds account-owned email state, challenges, recovery
proofs and grants, and audit history through the shared runner. Account
erasure cascades to these rows. The protected recovery inventory captures
the new tables under their account UUIDs. Invite-bound email challenges
without an account expire after ten minutes and are owned by the
installation until consumed or pruned. Preserve `system/invite.key`,
which also provides separate derived keys for account proofs and sealed
provider credentials. Auth schema 13 adds provider definitions with
sealed credentials, issuer-bound identity links and session origins.
Definitions belong to the installation. Links and session origins belong
to the account.

Updating a schema-11 installation to these releases requires an
incompatible handoff because the old protected recovery table inventory is
closed. The current production baseline already includes these versions. Take the full restore
point with explicit owner approval and use the locked deployment
coordinator for any remaining schema-11 installation. A v11 process must
refuse the newer authority rather than discard unknown records.
