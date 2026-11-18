# Data-preserving upgrades

Calternal-AI's first production baseline is the exact commit and image digest
that passes the release-to-release gate. The schema numbers below are the
stage 1 baselines. Preview data predates the supported baseline.

## Durable inventory

Paths with IDs always use account or space UUIDs. SQLite WAL belongs to its
database until checkpointed. SQLite SHM, locks and sockets are runtime state.

| Root and relative paths | Class | Writer and baseline marker |
| --- | --- | --- |
| Appdata `system/auth.db` | Authority | Auth, SQLite 11. Accounts, passkeys, sessions, roles, memberships, invites, approvals, machine identity and grants, erasures, key fingerprints and protected receipts. |
| Appdata `system/machines.db` | Authority | Machine placement, SQLite 5. Leases, fences, drain, work locations, discard and erase delivery. |
| Appdata `spaces/<id>/content.db` | Mixed | Content, SQLite 3. Item identity, revisions, pending placement, trash and tombstones, kinds, plugin controls. Listing, labels and links are derived. |
| Appdata `spaces/<id>/index.db`, `secrets/spaces/<id>/head` | Authority | Space journal, SQLite 2 and binary CHW2. Journal, epoch, receipts and independent head witness. |
| Appdata `spaces/<id>/jobs.db` | Authority | Jobs, SQLite 2. Admission, retry and in-flight state, checkpoints, event outbox and retention. |
| Appdata `accounts/<id>/input.db`, `content.db`, `agent-settings.db` | Authority | Account input, content and jobs. SQLite 2 for drafts and pending input, 1 for protected collections and 1 for retry settings. |
| Appdata `accounts/<id>/secrets.db`, `spaces/<id>/secrets.db` | Authority | Secrets, SQLite 1. Sealed values, pending prompts, audit and key state. |
| Appdata `system/invite.key`, account and space `secret.key` | Authority | Auth and secrets. Fixed 32-byte payloads with v1 key derivation and ciphertext binding. Preserve bytes, ownership and mode with their database. |
| Appdata `installed-plugins.sqlite3` and `installed-plugin-workers/` | Mixed | Package store, SQLite 1. Installed and retained archives, digests, approvals and revision counters are authoritative. Manifest API version 1 describes the package contract. Extracted worker copies and development drafts are disposable. |
| Appdata `system/content-index.db`, `spaces/<id>/search-v2.db`, `wallpapers/` | Derived | Content routing, SQLite 1. Retain the exact route catalogue across upgrades. Search uses SQLite and per-space generation 2. Wallpaper cache follows its rendition catalogue. |
| SSD space `Notes/`, `AI/Chats/`, Trash, registered visible and hidden content | Authority | Content writes `calternal-id` and a `calternal-note/v1` envelope for Markdown without its own format tag. Thread writers use `calternal-thread/v1`. Space Collections use `calternal-collection/v1` JSON for their structured membership. Other provider or plugin formats and opaque bytes follow their own contracts. |
| SSD space `AI/AGENTS.md`, `AI/Skills/*/SKILL.md` | Authority | Space instructions and the person write raw Markdown. Skill descriptors use the external Agent Skills format and may be provisioned on disk. These formats pass through byte-for-byte. |
| SSD `spaces/<id>/.calternal/settings.json` | Authority | The plugin settings adapter writes `calternal-plugin-settings/v1`. JSON preserves typed nested values, including unknown settings during an older writer’s update. |
| SSD automation definitions, account shortcuts, themes and other settings files | Authority when introduced | No durable server writer in this lane. Use `calternal-shortcuts/v1` for account shortcuts and `calternal-theme/v1` for themes. Automation definitions, custom components and starred prompts need their own versioned formats when their writers land. |
| SSD `.calternal/recovery/snapshot.json` and `log-*.bin` | Authority | Recovery coordinator writes v3 snapshots and checksummed frames, paired with the auth DB witness. |
| HDD originals and each root's `.calternal-install-id` | Authority | HDD originals are opaque. There is no general HDD placement writer today. Roots writes `calternal-install-root/v1` with installation UUID and role. |
| Machine `credentials`, `pending-enrollment/credentials`, and `machine-handoff/{request.json,enrolled}` | Authority during use | Machine enrollment writes `calternal-machine-credentials/v1`. Both roles use `calternal-machine-enrollment/v1` for the handoff. Protected files stay on the machine. |
| Machine `sandboxes/accounts/`, worker/container state | Mixed | Sandbox, harness and person write per-space homes, transcripts, account provider login and thread workspaces. Provider bytes are opaque. Worker containers, temporary login work and execution state are disposable. |
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
recovery command. A full restore point copies regular files and refuses a
nonregular entry, such as a workspace symlink, before any migration.

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
