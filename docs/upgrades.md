# Data-preserving upgrades

Calternal-AI's first production baseline is the exact commit and image digest
that passes the release-to-release gate. The schema numbers and generations
below are that first production baseline. Preview data predates it and is
refused rather than migrated.

## Durable inventory

Paths with IDs always use account or space UUIDs. SQLite WAL belongs to its
database until checkpointed. SQLite SHM, locks and sockets are runtime state.

| Root and relative paths | Class | Writer and baseline marker |
| --- | --- | --- |
| Appdata `system/auth.db` | Authority | Auth, SQLite 11. Accounts, passkeys, sessions, roles, memberships, invites, approvals, machine identity and grants, erasures, key fingerprints and protected receipts. |
| Appdata `system/machines.db` | Authority | Machine placement, SQLite 6. Leases, fences, drain, work locations, discard and erase delivery, and the durable stop record that keeps an erased worker's machine slot reserved until its stop completes. |
| Appdata `spaces/<id>/content.db` | Mixed | Content, SQLite 5. Item identity, revisions, pending placement, trash and tombstones, kinds, plugin controls. Listing, labels and links are derived. |
| Appdata `spaces/<id>/index.db`, `secrets/spaces/<id>/head` | Authority | Space journal, SQLite 2 and binary CHW2. Journal, epoch, receipts and independent head witness. Receipts are pruned after the operator's retention (7 days by default) only on content's command, with every receipt of a surviving outbox intent pinned. The fence adds no stored state. |
| Appdata `spaces/<id>/jobs.db` | Authority | Jobs, SQLite 3. Admission, retry and in-flight state, checkpoints, event outbox and retention. |
| Appdata `accounts/<id>/input.db`, `content.db`, `agent-settings.db` | Authority | Account input, content and jobs. SQLite 2 for drafts and pending input, 1 for protected collections and 1 for retry settings. |
| Appdata `accounts/<id>/onboarding.db` | Authority | Account onboarding, new SQLite 1 baseline through the shared runner. The production baseline has no marker. Missing files read as unfinished. The completion marker rides with account settings and is removed by account erasure. |
| Appdata `accounts/<id>/secrets.db`, `spaces/<id>/secrets.db` | Authority | Secrets, SQLite 1. Sealed values, pending prompts, audit and key state. |
| Appdata `system/invite.key`, account and space `secret.key` | Authority | Auth and secrets. Fixed 32-byte payloads with v1 key derivation and ciphertext binding. Preserve bytes, ownership and mode with their database. |
| Appdata `installed-plugins.sqlite3` and `installed-plugin-workers/` | Mixed | Package store, SQLite 1. Installed and retained archives, digests, approvals and revision counters are authoritative. Manifest API version 1 describes the package contract. Extracted worker copies are a cache reused across restarts only when they match the archive bytes, so they carry no version. Development drafts are disposable. |
| Appdata `system/content-index.db`, `spaces/<id>/search-v2.db`, `spaces/<id>/search-messages-v1.all.db`, `wallpapers/` | Derived | Content routing, SQLite 1. Retain the exact route catalogue across upgrades. Item search is SQLite 2 at generation 3. Message search, with its postings and thread-location index, is SQLite 1 at generation 2. Wallpaper cache follows its rendition catalogue. |
| SSD space `Notes/`, `AI/Chats/`, Trash, registered visible and hidden content | Authority | Content writes `calternal-id` and a `calternal-note/v1` envelope for Markdown without its own format tag. Thread writers use `calternal-thread/v1`. Space Collections use `calternal-collection/v1` JSON for their structured membership. Other provider or plugin formats and opaque bytes follow their own contracts. |
| SSD space `AI/AGENTS.md`, `AI/Skills/*/SKILL.md` | Authority | Space instructions and the person write raw Markdown. Skill descriptors use the external Agent Skills format and may be provisioned on disk. These formats pass through byte-for-byte. |
| SSD `spaces/<id>/.calternal/settings.json` | Authority | The plugin settings adapter writes `calternal-plugin-settings/v1`. JSON preserves typed nested values, including unknown settings during an older writer’s update. |
| SSD automation definitions, account shortcuts, themes and other settings files | Authority when introduced | No durable server writer in this lane. Use `calternal-shortcuts/v1` for account shortcuts and `calternal-theme/v1` for themes. Automation definitions, custom components and starred prompts need their own versioned formats when their writers land. |
| SSD `.calternal/recovery/snapshot.json` and `log-*.bin` | Authority | Recovery coordinator writes v3 snapshots and checksummed frames, paired with the auth DB witness. |
| HDD originals and each root's `.calternal-install-id` | Authority | HDD originals are opaque. There is no general HDD placement writer today. Roots writes `calternal-install-root/v1` with installation UUID and role. |
| Machine `credentials`, `pending-enrollment/credentials`, and `machine-handoff/{request.json,enrolled}` | Authority during use | Machine enrollment writes `calternal-machine-credentials/v1`. Both roles use `calternal-machine-enrollment/v1` for the handoff. Protected files stay on the machine. |
| Machine `sandboxes/accounts/`, worker/container state | Mixed | Sandbox, harness and person write per-space homes, transcripts, account provider login and thread workspaces. Provider bytes are opaque. Worker containers, temporary login work and execution state are disposable. MicroVM mode keeps each account's tree in `microvm/accounts/<account>.ext4`; the ext4 label `calternal-v1` versions the disk layout and an unknown label is refused untouched. `microvm/run/` and `vmm.lock` are runtime state. |
| Machine operator files for microVMs | Operator | The guest image is pinned by its manifest digest in the machine config. The machine config rejects unknown fields. The egress policy's `.installed` receipt is bound to the boot and fails closed, so it is reinstalled rather than migrated. |
| Appdata `system/upgrade-intent.json` and role-local `.calternal-upgrade/` | Temporary authority | Deploy writes the private `calternal-upgrade-intent/v1` decision and one restore point. The manifest binds installation, both image digests and the raw file proofs. The gate supplies `calternal-upgrade-proof/v1` evidence for the two images. |
| Appdata `startup.json`, `setup.token`, operator and release config | Mixed | Server startup publishes `calternal-startup/v1` and a replaceable setup code. Auth retains its hash and expiry. Operator owns the pinned installation ID and deployment secrets. Deploy owns image IDs, units and release pointers. |

## Forward changes

The shared runner in `calternal-store` ports the existing app's
`BEGIN IMMEDIATE` and `PRAGMA user_version` sequence. A current store pays one
version read. A store with pending steps rechecks under the write lock and
commits all steps and their version changes together. Keep the first shipped
schema and every shipped step fixed. A version outside the declared range
fails before schema changes. The deployment gate will admit a newer version
for an older release only under the explicit, tested compatibility policy.

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

### How to add a migration

1. Identify the writer and classify the change. For an authoritative SQLite
   store, find its `calternal_store::migrations::Schema` declaration. The
   writer and the deployment inventory must use that same descriptor. Keep
   the baseline number, initial batches, and previously shipped steps fixed.
2. Append one SQL batch for the next version. The runner derives the version
   from the number of steps and writes `PRAGMA user_version` in the same
   immediate transaction as the batch. Leave transaction control and version
   writes to the runner. Include any required deterministic backfill and
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

The space content migration from SQLite 4 to 5 adds
`thread_revision_patches` to the existing descriptor. It leaves every old SQLite snapshot and v1 Markdown
file unchanged. A new checkpoint records only its bytes and the offset and
hashes needed to place or read it. The current reader composes patches over
their immutable base snapshot. The first new patch does require a writer
that understands the new representation. The first patched commit retires a
rollback marker that would let an older writer open those heads.

The jobs migration from SQLite 2 to 3 adds `thread_send_receipts`. A saved
thread turn and its admission receipt now share the job transaction. The
receipt stores the request-key hash and job ID until the journal retention
deadline, including after job history eviction. Earlier receipts remain
readable from the journal. The first new receipt retires an older-writer
rollback marker because that writer cannot check the new receipt.

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
recovery command. Ordinary additive updates snapshot only changed stores
and their witnesses. A full restore point copies every regular file in the
appdata, SSD, HDD and machine roots and skips runtime sockets and locks. It
refuses a symlink or other special file anywhere in those roots, including a
thread workspace's `node_modules/.bin` link. The refusal comes after services
stop and before any migration, names the entry, and the deployment restarts
the previous release on unchanged data. A person sees a short reconnect, and
their workspace, including the link, stays in place. The incompatible update
does not proceed until the restore point can preserve links.

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
The measurement covers the server process and storage. Measuring Quadlet,
machine drain, Podman, and proxy overhead remains part of qualifying the
full deployment budget.

During a reconnect, the client retains queued sends and delivers them after
the server is reachable again. Requests that already received a durable
receipt replay using the same request ID.
