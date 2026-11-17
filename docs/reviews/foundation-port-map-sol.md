# Foundation port map: existing Calternal identity and storage

Read-only review, 2026-11-16. No product or architecture choice is made here. Port verdicts describe reusable implementation, not approval to deploy it.

## Evidence and limits

Existing repository: `/home/kayg/Developer/calternal`, **`origin/dev` = `ea57b84b9c9e40e8b67b7d1e1f1805915123b040`**. All existing-app citations below use `path:line` **at that revision**, even when a source comment describes an older layout. Reproduce with `git -C /home/kayg/Developer/calternal show origin/dev:<path> | nl -ba`. No working-tree source was used there; only git object/ref inspection. No builds, tests, checkout, migration, production access, commits, pushes, subagents or external messages were performed. Test citations mean test code exists and was inspected, not that it passed in this lane.

Rewrite: `/home/kayg/Developer/Calternal-AI`, branch `feat/overnight-preview`; inspected its current working files, including ongoing uncommitted auth/authz changes. Read `AGENTS.md`, architecture proposal §§2 and 4, inheritance audit, morning decisions, and `crates/server/src/{auth.rs,authz/mod.rs,store.rs}`. These working-file observations can change independently of the pinned old revision.

**Established constraints:** reuse the existing layout and inherited safety contracts; SQLite is rusqlite. Accounts authenticate; spaces own content/history, have Reader/Editor/Manager memberships, and can receive live resource shares. Account-private profile/authenticators/installations are protected and never become shared merely because a space is shared. Canonical content is charged once to its owning space; references do not duplicate usage and independent copies do. These are settled by the request and rewrite architecture/inheritance audit, not questions reopened here. The distinction between space Manager and the separate ownership invariant remains important.

**Main finding:** old `origin/dev` supplies substantial identity, journal and filesystem primitives, but its person=Home authority shortcuts cannot be copied unchanged. It implements the earlier HDD Home + SSD state arrangement. ADR 0062's amendment and FO1 are accepted requirements beyond that implementation. Replacing missing auth state with an empty database would weaken current safety and would not implement FO1.

## Rewrite baseline being replaced

- `crates/server/src/auth.rs:20` declares fixed spaces; `:37` exposes synchronous `Directory::{authenticate,role,describe}`; `:81` creates owner/guest fixture memberships; `:119` creates raw development-token files. This is neither persistent membership nor passkey/OIDC sign-in. The old authenticator is asynchronous, so its bounded database worker cannot simply be called synchronously inside `Directory::role` without an adapter/contract change. Retain identity-versus-current-authority separation.
- `crates/server/src/authz/mod.rs:49` explicitly binds operations to Reader/Editor/Manager; `:122` and `:138` check current membership before use/replay/delivery. Retain those guarantees when substituting stored accounts and memberships. They do not yet express individual-resource grants or ownership transfer.
- `crates/server/src/store.rs:121` opens one root, `.calternal.lock` and `state.sqlite3`; `:175` defines notes, versions, receipts and plugin_state; `:219` writes `spaces/<space>/Notes/<uuid>.md`. It commits FULL SQLite then materializes before acknowledgement, blocks detected outside changes, and explicitly excludes the check/rename race. Its `:29` disclaimer is accurate: it does not supply inherited tiering or file-only recovery. This preview hierarchy is not authority to replace the old layout. **Do not take** the fixed-principal/global-database implementation as the production foundation; keep applicable authorization, receipt and conflict fixtures as regressions.

## 1. Exact layout on origin/dev, and the separate accepted target

`D` is resolved `--data` / `CALTERNAL_DATA`; `S` is `--state` / `CALTERNAL_STATE`, defaulting to D. They are installation-wide roots, not a single person's directory. `u` is a stable user UUID. Thus D is the collection of Homes and **`D/users/u` is one Home**. Single-root compatibility is D=S; it is not a different schema.

```text
D/                                      # original user files, normally HDD
├── .calternal-install-id                # installation UUID plus newline
├── .lock                               # held for process lifetime
└── users/
    └── u/                              # this person's Home
        ├── Notes/                      # useful filenames/subfolders, not UUID-only exports
        │   └── <relative-path>.md
        ├── Trash/
        │   ├── <note-id>.md
        │   ├── <note-id>.json           # restore metadata
        │   ├── <note-id>.task.json      # when task metadata applies
        │   └── <note-id>.links.json     # when link metadata applies
        ├── Photos/YYYY/YYYY-MM-DD/<filename>
        ├── Documents/YYYY/YYYY-MM-DD/<filename>
        ├── backdrops/<uploaded-content-name>
        ├── .calternal/settings.json
        ├── prefs.json                  # legacy input; not a second preferred settings file
        └── <other user files/folders>
S/                                      # app state, normally SSD
├── .calternal-install-id                # same installation UUID as D
├── .lock                               # separate lock when canonical S != D
├── system/
│   ├── auth.db                         # global auth/security + sharing journal
│   ├── invite.key                      # exactly 32 bytes; mode 0600
│   └── admin.sock                      # runtime socket; fallback below
├── cache/unsplash/                      # rebuildable upstream image cache
└── users/
    └── u/
        ├── index.db                    # journal, request dedupe, publication receipts
        ├── notes/
        │   ├── history.db              # durable edits, pending writes, jobs, companions
        │   ├── meta.db                 # list metadata
        │   ├── ids.jsonl               # current dev ID/path implementation
        │   └── scan-state.tsv
        ├── search/search.db
        ├── attachments/
        │   ├── index.db
        │   └── pending/<boot>-<upload-id> # records Home-relative temp location
        ├── contacts/raw/<id>.vcf        # current existing contact-card copy
        ├── carddav.db                  # optional derived protocol cache
        ├── carddav-rebuild.db          # transient derived-cache rebuild image
        ├── carddav-rebuild.lock        # serializes that rebuild
        └── tasks/order.db              # optional app order, not merely derived file metadata
K/                                      # OPTIONAL --secrets / CALTERNAL_SECRETS
└── users/u/
    ├── token.key                       # signed sync-token key
    └── head                            # restore-detection high-water witness
```

SQLite `-wal` / `-shm` companions live beside **each** database while applicable. Entries are created on demand by enabled modules; the tree does not mean every account has every file. Upload bytes are staged adjacent to final files on D using `.calternal-tmp-<upload-id>`, with the recovery marker on S. Generic writes also use `.calternal-tmp-*`; recoverable settings exchange uses `.calternal-tmp-intent-settings.json` beside the settings file (`fsio/src/swap.rs:163–186`). CardDAV rebuild siblings are transient (`carddav/src/cache/rebuild.rs:27–31`). These are transient paths, not extra content roots.

Code evidence: `crates/calternal/src/storage.rs:81,85,212,236`; `crates/auth/src/lib.rs:178`; `crates/engine/src/runtime.rs:76`; `crates/notes/src/home.rs:319`; `crates/notes/src/lib.rs:362`; `crates/notes/src/trash.rs:95,100,105,291`; `crates/attachments/src/lib.rs:238,243`; `crates/attachments/src/stage.rs:158,168,192`; `crates/prefs/src/store.rs:158,164,169`; `crates/contacts-actions/src/cards.rs:31`; `crates/carddav/src/book.rs:28`; `crates/tasks/src/order.rs:4`; `crates/backdrops/src/lib.rs:132`; `crates/backdrops/src/uploads.rs:98`; `crates/store/src/lib.rs:196,625`. The documented summary is `docs/storage.md:9–30`; it abbreviates `notes/...` and other app paths beneath each user's state directory, not globally beneath S.

The admin helper calls its argument `data`, but main passes **state**, `crates/calternal/src/main.rs:165`. For a long socket path it uses `$TMPDIR/calternal-admin-<16-hex-hash-of-canonical-state>/admin.sock`; the fallback directory is private and ownership-checked (`crates/calternal/src/admin.rs:74–102`). K is a separate, optional restore witness location, not the second tier or a replacement for `system/invite.key`; comments require K outside data backups/snapshots (`crates/store/src/lib.rs:185`). Without it main warns restore detection is disabled (`crates/calternal/src/main.rs:120`). Do not fold K into an ordinary coordinated state backup and claim the same rollback detection.

### What ADR 0062 amendment and tiering v3 require beyond dev

The **fixed target** is three roles/roots: SSD Home tree, HDD Home tree and separate SSD app state. Both Home trees retain the same relative `users/u/...` directory shape and real filenames, with empty folders on both. Claim-validated text belongs on SSD; unclaimed text, media and attachments on HDD; extension, app folder and file size do not select a tier. State and SSD Home may share a filesystem but have separate directories/ownership roles. No app-created hardlinks, stubs, content-addressed original store or link farm replaces real files. (`docs/adr/0062-user-files-on-hdd-app-data-on-ssd.md:84–166`.) This review does not invent new flag names or a new tree hierarchy for spaces.

| Accepted requirement | Verified dev status / work still needed |
|---|---|
| Lock all three roots; installation UUID **and root roles**; reject swapped/missing mounts before writes | `storage.rs:85–131` only knows D and S, compares UUID text and canonical-root equality. It neither pins expected SSD/HDD role nor performs full mount/device/symlink-substitution validation. `require_root` uses `is_dir`; its safety must not be overstated. |
| One joined namespace, durable two-tree folder operations/intents, replay before affected paths are served | Current Home has one Notes directory and one Trash directory. Existing bulk/history and journal primitives are reusable, but not a completed two-tree namespace transaction. |
| Physical dedupe after independent durable upload, per filesystem; XFS reflink probes, ordinary-copy ext4 fallback; no cross-user signal | Existing attachments' same-Home hash hint is not the amendment's server-wide, asynchronous physical dedupe. Preserve destination-scoped visible duplicate behavior and logical accounting. See ADR :168–216. |
| Verified copy-before-retire, byte/hash checks, durable progress and source write exclusion, preserve outside/open-inode writes | fsio supplies guards; no universal tier mover or all-format exclusion proof exists in dev. ADR :218–253 explicitly makes open-inode safety a gate. |
| Offline, lossless, resumable migration from single-root and existing split; verified inventory, all-root cutover and rollback | Existing `re_pair` only rewrites two UUID markers after limited checks; it does not migrate or prove arbitrary history matches a Home. ADR :255–289 requires more. |
| FO1 protected recovery metadata, externally pinned installation UUID, validated empty-state reconstruction | Dev refuses missing auth; no global Home security recovery snapshot was found in the inspected write paths. Detailed evidence below. |
| Ack-to-durable-Home maximum 1,000 ms; degradation/backpressure on failure | Existing SSD-history-before-file flow is implemented, but neither that structure nor a test name establishes this maximum. No `home-materialization-1s` implementation/gate was located in crates at dev. ADR :404–458 is a requirement. |

Tiering v3 is approved for implementation, not an implementation on dev (`docs/plans/tiering-v3.md:110–121`). Its additional work is one SSD current namespace index shared with Files; one compact active ID map with ID/path/owner-tree/generation; one pinned-root no-follow open, no alternate-tree probe; room-held references; watcher generations and bounded reconciliation; indexed tag orders updated/created/title and counts; bounded rebuild before affected results serve. It excludes pending overlays, identity-log startup replay and fallback per-request SQLite lookups (`:32–47`). Dev still loads Notes `ids.jsonl` (`home.rs:322`), so that is not proof of v3's shared current-table implementation. The map cap is 56 bytes/live entry and total namespace cache 64 MiB; qualification includes both 1M entries in one Home and ten active 100k Homes, 600-second idle release/reload, real bytes and strict paired faster-than-dev gates through 1M (`tiering-v3.md:49–90`). These gates are inherited, not measurements run here; the plan's four questions were already answered and should not be asked again.

### origin/feat/tiering-v5 (three-sentence comparison)

At `72b600edefa154a7e4f611fb884545fdc802177a`, `origin/feat/tiering-v5` adds Notes-specific mirrored SSD/HDD reads, writes, scans/watchers, promotion/rename/Trash integration, a queued background mover, race/crash fixtures, placement-observation helpers and split-tree performance harness modes (54 changed files relative to the merge base). Its `crates/notes/src/split_roots.rs:17–29,125–165` explicitly uses SSD precedence and HDD fallback with no per-file tier state, which differs from v3's one-selected-root reader; it is not evidence of the full all-format three-root/FO1 contract or passed performance gates. It is **not merged into origin/dev**: `git merge-base --is-ancestor origin/feat/tiering-v5 origin/dev` exits 1 and `git log origin/dev..origin/feat/tiering-v5` lists branch-only commits; this establishes local-ref ancestry, not remote PR status or squash-equivalence certification.

## 2. Module-by-module port ledger

“Public” below separates exported Rust interfaces from crate-private helpers; the latter are not new integration APIs. Auth table ownership maps to §3 (all in S/system/auth.db). Store tables map to §4 (all in S/users/u/index.db). Modules without their own SQL are explicitly identified. File protection refers to host authority, **not** a claim that every existing database is already chmod-0600 or sandboxed against hostile tenant code.

### crates/auth

All rows inherit `crates/auth/Cargo.toml:8` dependencies: old core/engine/server/fsio contracts, axum, tokio, rusqlite, webauthn-rs, OpenSSL, BLAKE3, base64/getrandom/uuid, serde/serde_json/schemars, URL/thiserror/tracing and Unicode normalization/security/casefold/category libraries. These are real coupling costs: do not import old Rust product `Plugin` implementations into the rewrite as tenant-replaceable auth. Re-expose protected operations through the new contracts. Tests use testkit, tower/body utilities, softpasskey, websocket helpers, attachments/backdrops and fsio test hooks (`Cargo.toml:39`).

| Module / public surface | Job, schema and protected I/O | Tests to carry | Verdict and exact space adaptation |
|---|---|---|---|
| `lib.rs:50–59,113,170–289` | Exports Auth, AuthConfig/errors, Account/People/Groups/SharingPlugin, Installation kinds, app-password/DAV types, create/find/recover, cookie middleware. Auth open/open_with_state, routes, first_owner_token, set_engine, sharing resolver and plugin accessors. Opens global auth/key, per-person usage and fenced sharing maps. | Auth integration suites below; wiring/isolation tests in calternal. | **Take and adapt:** keep account authentication global; inject a space directory/storage resolver rather than let account ID select content. Split usage and locks to owning space. Keep security owners protected. Old engine/set_engine/plugin adapters need replacements. |
| `db.rs:15–332,345–452` | Private Db/open_conn/run; v15 migrations, WAL/FULL/FK, busy timeout 5s, bounded 64-job dedicated blocking worker. Owns every table in §3; creates parent directories and opens auth.db. Does not itself enforce root pairing or private filesystem modes. | `tests/migration.rs:7`, `security_regressions.rs:424`; inline `db.rs:469`. | **Take and adapt:** reuse rusqlite worker and migration discipline; add separate account identity, space membership/ownership and target grants without reinterpreting users.role as a space role. Preserve existing tables during verified migration; add FO1 coordinated security publication. |
| `config.rs:8,66,106,143`; `passkey.rs`; `ceremony.rs:50–125` | AuthConfig parses origin/RP and trusted proxies; builds WebAuthn; bounded expiring ceremony storage with take-once/add-for-user checks. No independent SQL/files; resulting credentials use passkeys table. | Inline config/ceremony tests; `tests/login.rs:32`, `isolation.rs:106`, reliability proxy/prompt-flood tests. | **Take and adapt:** mechanics carry; bind ceremonies to account + purpose, not selected space. Origin/RP changes need explicit old-credential compatibility handling. Existing code supplies passkeys, not the rewrite's OIDC provider/linking implementation. |
| `token.rs`; `limit.rs:21–192` | Private random/token hashing helpers, plausibility checks and failure/admission limiters. No extra SQL/files. Sensitive tokens must stay out of outcomes/logs. | Inline token/limiter tests; `tests/abuse.rs`, `reliability_review.rs:51,139,171,514,848`. | **Take as is** for token/limiter algorithms and bounded primitives; keep account/source/credential budgets distinct. New space-level scheduling composes with them rather than changing token identity into space identity. |
| `setup.rs:38,79,117,171,261,324`; `invite_redeem.rs:10,82` | Exported create/find/recover operate on auth.db; private setup check/redeem consume tokens and create users/passkeys/session. Redemption uses an IMMEDIATE transaction, then creates D/users/u **after commit** (`setup.rs:393–399`). Tables users/setup_tokens/passkeys/installations, or invite tables/groups. | `tests/setup.rs`, `create.rs`, `migration.rs:7`; invites racing/expiry tests. | **Take and adapt:** recover the same account; enrollment and accepting a space invitation are different operations. Existing-account invitation must add a membership without another account/Home. New account setup must establish account-private records and an initial space through recoverable publication. No filesystem+SQLite atomicity is supplied by current post-commit mkdir. |
| `session.rs:24,62,119–255,259–398` | Exported Installation/InstallationKind; private create/identify/list/revoke/require_fresh; hashed tokens, expiry/touch, in-process revoke fence; separate DAV cache. Tables installations, plus sharing reset bookkeeping on stale use. Protected auth.db, no Home content file. | `tests/login.rs:11–296`, `host_cookies.rs:8,53,110`, `app_passwords.rs:909,978,1034,1153`; inline session tests. | **Take and adapt:** sessions remain account-owned. Resolve and recheck current membership/resource grants per operation and long-lived delivery. Authenticated account does not imply Home/space ownership. Add FO1 session/security recovery publication; do not describe cache-local revoke fencing as instantaneous cross-process revocation. |
| `passkeys.rs:66,94,119,177,201,261` | Private list/add/rename/remove and action specs; last-passkey deletion guarded transactionally. Tables passkeys/installations freshness. No separate files. | `tests/passkeys.rs:43,154,209,312,336`, `isolation.rs:50,106`. | **Take and adapt:** keys remain in protected account profile; never shared by space membership. Preserve interactive freshness and last-key invariant, while adding linked OIDC methods and protected recovery according to the settled policy. A space Manager gains no authority over another account's authenticators. |
| `http.rs:41,46,606`; `account.rs:45–134` | Auth routes setup/login/reverify/logout/me/app-password mint and Authenticator/cookie middleware; __Host-calternal_session, origin/proxy/cookie handling. Account plugin supplies account.installations/revoke/app-passwords/app-password-revoke plus passkey actions. Tables installations/passkeys/setup_tokens. | host_cookies, login, isolation, abuse, passkeys, reliability suites; `http.rs:793,807,934` inline tests. | **Take and adapt:** trusted account endpoints use rewrite operation/transport boundaries; `http.rs:707–775` already queries current installation kind, expiry/revocation and Notes credential scopes for long-lived revalidation; keep origin, duplicate-cookie and app-password route segregation. Account events must go to protected account sync/Activity, not a shareable space journal. UI work is outside this lane. |
| `app_password.rs:41,101`; `dav.rs:146–247` | Exported scope/password/DAV identity/gate/check and Auth mint/dav_gate. Scoped DAV and Notes credentials use installations + installation_notes_scopes. DAV/Notes capability families are independent; minted secret is not stored plaintext. | `tests/app_passwords.rs:117,221,262,291,336,399,447,615,645,727,870,978,1257`; inline modules. | **Take and adapt:** retain account credential custody and protocol limits, add explicit allowed space/resource targets. DAV URL account ID must not silently grant all member spaces. Resolve canonical owner space and effective grant at request and commit/delivery. |
| `roles.rs:11–112`; `people.rs:28–143` | Private capability-set lookup/require/change/can_see; exported People plugin exposes people.list/role plus quota/invite actions. Tables users/roles/role_capabilities/group_members. Last people-manager cannot be demoted; visibility is self, people.manage or shared group. | `tests/people.rs`, `groups.rs:7`, `security_regressions.rs:38,276,325`; calternal people isolation. | **Take and adapt:** keep deployment admin capabilities separate from per-space Reader/Editor/Manager and ownership. Add last-owner/transfer-before-leave enforcement per space. No global-admin content bypass. Old directory visibility is a policy input for the unresolved group/discovery choice, not an adopted space rule. |
| `groups.rs:52,89–253`; `group_list.rs:7,57` | Exported Groups plugin create/list/rename/delete/member; private listing helpers. Tables groups/group_members/invite_groups. Local/OIDC source and manager bit; local managers cannot edit provider-owned membership. No other files. | `tests/groups.rs:7`, `security_regressions.rs:325`; invite group tests. | **Take and adapt:** do not equate group manager bit with space Manager or turn each group into a space automatically. Keep identities/membership provenance; explicit mapping/grants and revocation rules must be specified (Q3). Current source='oidc' data model is not proof of OIDC login integration. |
| `invites.rs:26–208`; `invite_create.rs:22`; `invite_key.rs:12–120`; `invite_redeem.rs:82` | Private invite specs exposed by People: create/list/copy/expiry/revoke. Tables invites/invite_groups/users/group_members. 256-bit secret, separately derived keyed BLAKE3 lookup, AES-GCM encrypted recoverable copy. Key is S/system/invite.key 0600, atomically published; refuse missing key when invite rows exist. Full candidate lookup + constant-time comparison even for malformed invite (`invite_redeem.rs:15–78`). | `tests/invites.rs:19,141,250,295,453,548,592`; `security_regressions.rs:96,184,222,236,521,561,597`; inline key/redeem tests. | **Take and adapt:** keep cryptography, generic refusals, expiry recheck and one-winner transaction; distinguish server admission from space invitation, specify role/space target and existing-account redemption. Person quota/group payload cannot simply become a second membership policy. Invite secrets remain protected, including FO1 recovery copies. |
| `quota.rs:17–104` | Private people.quota action; Auth implements UploadPolicy::admit. Per-user placement mutex, ensure recount off auth worker, maintained bytes + users.quota_bytes check. No new file/table. | quota_counter:8,35; security_regressions:367; reliability_review:254,349,584,720,779,805. | **Take and adapt:** authorize actor first, reserve/check canonical **owning space** limit and serialize all its writers across members. References cost no second copy; independent copies count. Replace users.quota_bytes ownership with explicit space accounting; note-save behavior above quota remains preserved. Retention breakdown is Q5, not permission to discard history. |
| `sharing.rs:60,374,861,1305,1705–2840`; `sharing_inbox.rs` | Exported SharingPlugin + Auth's NoteSharing implementation: grant/list/revoke, access/resource resolution, admission, delivery fences, lifecycle, controls/acks and reset/fanout; inbox hide state. Tables note_shares through share_fanout_deliveries in §3. Writes auth.db and emits recipient journal publications through engine; room eviction callback. Per-(owner person,note) and per-recipient fences. | Inline `sharing.rs:2876` suite; `crates/calternal/src/isolation.rs` sharing tests and `isolation/grants.rs:152,364`; store publication receipt test. | **Take and adapt:** retain sequencing, idempotent fanout, purge/reset fences and current-delivery checks; owner key becomes canonical owning space, actor stays account, targets include account **or space**. Remove caller.user==owner shortcuts and unshared-owner cache bypasses; membership revocation must invalidate effective access for all deliveries. Do not duplicate canonical note bytes for recipient spaces. Personal hide/ack/installation state stays account-private. Preserve the existing direct-share history visibility boundary rather than exposing pre-grant drafts (`core/sharing.rs:405–426`). |

The `lib.rs:22–23` comment “per-user data never lives here” must be read narrowly: the actual v11–15 auth schema stores note event data, grants, control and fanout state. This is not an auth-only database that can be ported while discarding its sharing tables.

### Store, filesystem, pairing and adjacent owners

| Module / surface | Schema, protected files, dependency and tests | Verdict / changes for spaces |
|---|---|---|
| `crates/store/src/lib.rs:34–114,172–517`; schema/read/writer | UserStore open/open_with/open_secured, commit/commit_once/idempotency/since, subscribe/listeners/head/token_at/position, restore watches/save activity/flush_highwater. §4 schema in S/users/u/index.db and SQLite companions; optional K/users/u/{token.key,head}. Dependencies: core, fsio, rusqlite, tokio, BLAKE3, serde_json, uuid, tracing (`Cargo.toml:8`). Single 64-command writer; publication receipts use FULL, ordinary journal NORMAL (`writer.rs:19,65–110`; `schema.rs:58–65`). Tests `src/tests.rs:19,39,72,103,119,169,185,221`. | **Take and adapt:** keep durable receipt/ordered journal mechanisms and queue bounds; route **content** journals by space and account events separately. Qualify request keys by actor/installation/scope to prevent two members replaying each other's response. Do not infer tenant isolation from separate files. NORMAL ordinary journal commits are not a universal power-loss-durable ack primitive. |
| `store/src/{token,proof,highwater,restore,shared}.rs`; lib :185,320–469,625 | Signed tokens, bounded proof/issued-token caches, epochs and durable high-water detection; no additional SQL beyond §4. Key/witness outside snapshots; epoch rotation and save-activity guards. Internal helpers except exported UserStore methods/guards. Tests `src/tests/restore.rs:111,174,257,317,354,386,409,480,517,559,741,826,850`; restore_failures:99,131,145,180,210,343; inline token/proof/highwater/shared. | **Take and adapt:** one canonical journal/epoch domain per space plus protected account domain; prevent equal seq/epoch tokens crossing domains. Existing code derives secret folder from store directory's basename; replace implicit person inference with validated root binding. Preserve independent witness semantics. Witness detects rollback, does not reconstruct auth, user files or history. |
| `crates/fsio/src/lib.rs:29–48,63–135,225–267,310–357` | Public atomic/mode/stream writes, safe rename/remove/sync/durable mkdir; DataLock acquire/acquire_existing/path; bulk write outcomes and prepared directory publication. No SQLite. Caller-specified files, temp siblings, root .lock; mode variant for sensitive metadata. Depends arc-swap, tracing, unicode-normalization, rustix fs on Linux/Android/Apple (`Cargo.toml:11`). Tests `src/tests.rs:22,42,59,73,107,150,223,258,390,461,485,582,623,651,684,708,731`. | **Take as is** for generic primitives and their platform failure behavior. Use acquire_existing after root validation; ordinary write_atomic is for app-owned metadata, not an authorized-content API. Host authorization and allowed root descriptors remain the caller's responsibility. This crate alone is not a tenant sandbox. |
| `fsio/src/swap.rs:31–119,200`; `directory.rs`; `bulk.rs`; `names.rs` and names/directory | Prepared new/metadata/retain_replaced_at/place/place_exact/place_exact_recoverable, recover_exact; guarded exchange and inode retention; batch/directory publication; NFC/case-fold names/DirectoryNames. No SQL; temp/exchange intents/displaced files under supplied directories. Same fsio dependencies. Main tests above, inline swap/names tests; names/directory.rs:199. | **Take as is** for generic guarded placement/name machinery. Keep displaced inode and crash recovery behavior; do not replace it with check-hash-then-rename. Extending one-file recovery to cross-root namespace/security snapshots is additional work, not an existing guarantee. |
| `fsio/src/beneath.rs:26–177,279`; `paced.rs:30–60` | Folder open/open_sub/open_file/names/remove/sync/create_temp; TempFile file/name/persist; plain_parts; paced copy/write_all/write_chunk. No SQL. Descriptor-relative no-follow operations and bounded disk work; platform gated. Inline beneath tests :306 and fsio tests. | **Take as is** for containment/pacing primitives. Host chooses an already-authorized canonical space root; never accept an account-provided filesystem path as authority. Applies to protected roots too; not all legacy callers already use these APIs. |
| `fsio/src/usage.rs:19–179` and usage/files | Public Usage new/with_state/ensure/background/bytes/recount/recount_with_after_walk; provide callback; mutation hooks. No SQLite. In-memory path-size map and published totals; walks D/users and registered state providers, excludes temp prefix, merges concurrent mutations. Dependencies as fsio, notably arc-swap. Tests usage.rs:355; usage/files.rs:102; auth reliability counters. | **Take and adapt:** replace first path-component-as-person inference with validated owning-space binding; count joined tiers once logically, excluding mover staging and physical dedupe savings. Split account-only charges from shared space charges; plugins report under host-owned accounting, not arbitrary quota exemptions. |
| `crates/calternal/src/storage.rs:13–131,178–260,265`; args/admin/main | Public prepare_for_server/prepare/re_pair/warn_rotational/shutdown_server. No schema; verifies nonempty auth file, matching .calternal-install-id and missing legacy history; owns D/S .locks and marker writes. Dependencies fsio DataLock/atomic writer, Notes shutdown, core InstallationId, tokio/tracing and standard I/O. Admin helper owns runtime socket; CLI requires confirmed offline re-pair. Tests `tests/storage_startup.rs:201,221,248,293,321,346,386,522,536,579`; shutdown isolation tests. | **Take and adapt:** preserve fail-closed/create/re-pair safeguards and lifetime locks, extend to all three fixed root roles, pinned installation identity, and FO1 validation before state creation. Never loosen auth refusal without the validated replacement recovery path. Scan all registered space bindings for state compatibility, not just users' history paths. Re-pair currently checks presence, not provenance. |
| `crates/core/src/{ids,action,journal,sharing,rooms}.rs`; `action.rs:18` | UserId/InstallationId, Caller/credential, ActionSpec/Outcome/ActionCtx, journal/sink/recorder, NoteSharing and room interfaces. No own SQL or files. Coupled to old typed operation/event and async/serialization contracts. Tests in core and engine/Notes consumers. | **Take and adapt:** split actor AccountId from owning SpaceId and canonical resource owner. Preserve request/install/run attribution and credential scope intersection. Reuse protocol semantics through rewrite contracts; do not wholesale import person=Home Caller or old product plugin registration. |
| `crates/engine/src/{lib,runtime,execute,sink}.rs`; runtime :43–91 | Engine execute/query/live/head/token/restore/recorder adapters; lazy runtime map keyed UserId; opens users/u/index.db. §4 SQL via store, no extra engine DB. Dependencies old core/store + tokio/JSON/BLAKE3 machinery. Tests engine/src/tests.rs:381,626 and tests/restore.rs; store order/dedupe tests. | **Take and adapt:** runtime/sink/inflight keys become explicit account versus space scopes; content authorization occurs before runtime selection, retry response and commit. Existing action handler state mutation and later journal commit are not one cross-database transaction (`execute.rs:159–218`). Coordinate protected security changes and receipts rather than assume the engine already does so. |
| `crates/server/src/{auth,api,access,live,rooms}.rs` | Public Identity/Authenticator authenticate+revalidate, NoAuth/DevUser, route inventory/classification; crate-private Signed extraction, action/journal HTTP and live room transports (`auth.rs:16–88`; `api.rs:18–104`; `access.rs:51–123`; `live.rs:106–114`; `rooms.rs:99`). No own SQLite/files: calls engine/store domains and room services. Direct dependencies are in Appendix C. Tests server/tests/{access,live,restore,signout}.rs. | **Take and adapt:** preserve credential-family segregation, denial status/retry headers, live revalidation, room generation/nonce fences and bounded queues. Journal/live currently select caller.user; use an authorized explicit space stream or protected account stream, and scope room keys by canonical owner/resource. The connection authenticate step never grants every member space automatically. |
| `crates/notes/src/{lib,home,index,history,scan,room/save,trash}.rs` | Home/Notes/History and recorder/sharing integration own durable history, pending saves/jobs/companions, ID paths, current views and Trash. Extra SQLite is notes/history.db/meta.db, **not** auth/index tables; history schema in history/schema.rs:9 onward. File writers use fsio and history executors; dependencies old core/engine/fsio/store-facing recorder, CRDT/history and indexing layers. Tests Notes inline/history/index, tests/kill9/restart_listing, calternal notes_crash/notes_crash_typing. | **Take and adapt:** keep durability machinery behind versioned host services; Home caches/rooms/jobs/history belong to space, author remains account. Remove owner equality fast paths (`home.rs:231,254,1019`; auth sharing :1771). App format behavior remains plugin/service scope; portable file content must not become SQLite-only preview materialization. Separate port review needed for all history schema, not a claim that index.db contains history. |
| `attachments/src/{lib,stage,place,sweep,serve,index}.rs` | Upload actions/HTTP, per-UserId stage/index/placement maps, adjacent temporary bytes and state pending markers; attachment index.db is app-specific, outside §§3–4. core/server/fsio, sniff/hash/index dependencies. Tests attachments/src/tests/{http,sniffing,index_flush}.rs; calternal/tests/attachments_crash.rs. | **Take and adapt:** root/index/staging locks and authorized serving key by canonical owning space; actor stays account. Carry sniff limits, crash sweep, portable Photos/Documents paths. Keep visible duplicate checks authorized/destination-scoped; legacy size+indexed-hash shortcut is not enough after a same-size outside rewrite (`place.rs`, inheritance audit). |
| `prefs/src/{lib,store}.rs`, `backdrops/src/{lib,uploads}.rs` | Prefs plugin/store all/set; JSON settings, legacy prefs, no prefs SQLite. Backdrop upload/store/cache endpoints use Home backdrops, per-user limits/locks and shared upstream cache; no auth/index schema. Dependencies old plugin/core/server/fsio + JSON/http/image plumbing. Tests prefs/src/{tests,store/tests}.rs and backdrops/src/tests.rs. | **Take and adapt:** personal layout/settings/install selections stay in protected account profile, not the initial space; shared defaults/app settings belong to space. Uploaded bytes need explicit attribution when reused as shared defaults. Preserve recoverable settings exchange and bounded upload/abort cleanup. No adoption of an account-profile directory is decided here. |
| `search/src/lib.rs:113–158`, `contacts-actions/src/{cards,write}.rs`, `carddav/src/book.rs:28`, `tasks/src/order.rs:4` | Search provider/index APIs use users/u/search/search.db. Cards stores original .vcf on S and history companions, CardDAV derives a per-user cache; Tasks keeps manual ranks in order.db. These app-specific schemas are outside the requested auth/index inventory. Depend on old Notes/core/fsio/rusqlite/provider/protocol layers; tests search/tests, contacts-actions tests/{correctness,kill9,review}, carddav/tests/journal, tasks/src/tests. | **Take and adapt:** space-owned content indexes and protocol views must filter effective access; account connected-provider credentials remain private. Preserve original opaque card bytes and history; state-only raw cards are not full FO1 Home recovery. Keep manual task order through migration (not a disposable cache). No generic blanket transplant of app databases into auth. |

## 3. Exact effective auth.db schema

`crates/auth/src/db.rs:16` is version **15**. The following is the complete application-table column inventory after v1–15, in column order. `T/I/B` = TEXT/INTEGER/BLOB; `!` = NOT NULL; `?` = nullable; PK and UNIQUE indicate declared keys (for non-STRICT legacy TEXT PRIMARY KEY tables, do not infer an extra explicit NOT NULL declaration). FK/check/index/trigger definitions are preserved verbatim in Appendix A. `host_cookie` was added in v6 and **dropped in v7**, so is absent today. Timestamps here are integers; the schema does not use the preview's RFC3339 strings.

| Table | Exact columns | Keys / evidence in db.rs |
|---|---|---|
| users | id T PK; name T!; username T!; created_at I!; role T! DEFAULT 'member'; quota_bytes I? | username UNIQUE COLLATE NOCASE; :19,74,92 |
| passkeys | cred_id B PK; user_id T!; passkey T!; created_at I!; last_used_at I?; name T! DEFAULT 'Passkey' | FK user_id→users; :25,57 |
| setup_tokens | hash B PK; purpose T!; user_id T?; created_at I!; expires_at I!; used_at I? | purpose first-owner/recover, FK users; :33 |
| installations | id T PK; user_id T!; kind T!; name T!; token_hash B!; created_at I!; last_seen_at I!; expires_at I?; revoked_at I?; scopes T! DEFAULT ''; assertion_at I?; registration_session I! DEFAULT 0 | token_hash UNIQUE; kind browser/native/cli/agent/dav; FK users; :41,131–171 |
| roles | name T PK | :62 |
| role_capabilities | role T!; capability T! | PK(role,capability), FK roles; :63 |
| groups | id T PK; name T!; source T! DEFAULT 'local' | source local/oidc; :93 |
| group_members | group_id T!; user_id T!; manager I! DEFAULT 0 | PK(group_id,user_id), FKs groups/users, manager 0/1; :98 |
| invites | id T PK; created_by T!; hash B!; secret B!; created_at I!; expires_at I!; used_at I?; revoked_at I?; quota_bytes I?; plan T?; lookup_version I! DEFAULT 0 | hash UNIQUE, FK created_by→users, nonnegative/null quota; :105,126 |
| invite_groups | invite_id T!; group_id T! | PK(invite_id,group_id), cascade FKs; :117 |
| installation_notes_scopes | installation_id T!; scope T! | PK(installation_id,scope), FK installations, checked notes scopes; :188 |
| note_shares | id T PK; owner_id T!; note_id T!; last_note_seq I! DEFAULT 0; created_at I!; creator_installation T?; deleted_at_seq I? | UNIQUE(owner_id,note_id), FK owner users/creator installations; :197,314 |
| share_grants | id T PK; share_id T!; person_id T!; permission T!; grant_start_seq I!; revoked_at_seq I?; created_by T!; revoked_by T? | FK share/users; active (share_id,person_id) unique partial index; permission read/edit; :206 |
| note_journal | owner_id T!; note_id T!; note_seq I!; event_id T!; kind T!; item T!; data T!; actor_id T!; installation_id T?; request_id T?; at_ms I! | PK(owner_id,note_id,note_seq), UNIQUE(owner_id,event_id), composite FK note_shares + actor/install FKs; :220 |
| share_fanout | owner_id T!; note_id T!; note_seq I!; created_at I! | PK(owner_id,note_id,note_seq), composite FK note_journal CASCADE; :236 |
| recipient_control | id T PK; recipient_id T!; share_id T!; note_seq I!; kind T!; created_at I! | UNIQUE(recipient_id,share_id,note_seq,kind), users/share FKs; kind purge/delete/baseline/loss-of-edit; :247 |
| recipient_control_acks | control_id T!; installation_id T!; acknowledged_at I! | PK(control_id,installation_id), cascade FKs; :256 |
| recipient_share_state | recipient_id T!; share_id T!; hidden I! DEFAULT 0 | PK(recipient_id,share_id), users/share FKs, hidden 0/1; :262 |
| share_fanout_acks | owner_id T!; note_id T!; note_seq I!; installation_id T!; acknowledged_at I! | PK(owner_id,note_id,note_seq,installation_id), fanout/install cascade FKs; :273 |
| recipient_share_fences | recipient_id T!; share_id T!; note_seq I! | PK(recipient_id,share_id), FKs; :285 |
| sharing_installation_resets | installation_id T PK; required_at I! | FK installations CASCADE; :291 |
| sharing_reset_cursors | installation_id T!; owner_id T!; note_id T!; note_seq I! | PK(installation_id,owner_id,note_id), install/users FKs; :301 |
| share_fanout_deliveries | owner_id T!; note_id T!; note_seq I!; recipient_id T!; delivered_at I! | PK(owner_id,note_id,note_seq,recipient_id), fanout/users FKs; :320 |

Tables from installation_notes_scopes onward are STRICT; earlier tables are not declared STRICT. There are **23 application tables**, no sessions table distinct from installations, no spaces table, no OIDC identities table, no Home recovery snapshot table, and no usage-counter table. Quota usage is maintained outside SQLite. The schema's source tag `oidc` does not implement authentication. Standard SQLite bookkeeping is not another application schema.

ADR 0059 (`docs/adr/0059-invites-and-storage-quota.md:7–41`) settles seven-day default single-use links, optional local groups/quota/plan payload, encrypted copyable secrets, final-transaction revalidation, counted Trash/backdrops, excluded temporary uploads, recount before post-restart admission, and note saves above quota. Its plan name is only a label, not access or payment authority; the rewrite must not infer billing or space grants from it.

The role triggers are consequential: existing users get member by default; first person becomes admin; role existence/deletion is trigger-checked; seed admin has people.invite, people.manage, groups.manage and server.settings. These server-wide capabilities are **not** the rewrite's per-space roles. The v11 installation scope triggers replace v9 triggers; read the final versions rather than preserving obsolete equivalence checks accidentally. `registration_session` is an INTEGER flag without an explicit 0/1 CHECK; do not claim a constraint that is absent.

## 4. Exact users/u/index.db schema and journal semantics

`crates/store/src/schema.rs:13–40` has three migrations, so user_version=3 on a new current store. All four application tables are STRICT:

```sql
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;
CREATE TABLE journal (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    at_ms INTEGER NOT NULL,
    kind TEXT NOT NULL,
    item TEXT NOT NULL,
    data TEXT NOT NULL,
    installation TEXT,
    request_id TEXT
) STRICT;
CREATE TABLE idempotency (
    key TEXT PRIMARY KEY,
    action TEXT NOT NULL,
    payload_hash TEXT NOT NULL,
    result TEXT NOT NULL,
    seq INTEGER NOT NULL,
    at_ms INTEGER NOT NULL
) STRICT;
CREATE INDEX idempotency_at ON idempotency(at_ms);
CREATE TABLE publication_receipts (
    key TEXT PRIMARY KEY, digest TEXT NOT NULL, seq INTEGER NOT NULL
) STRICT;
```

SQLite also creates its internal `sqlite_sequence(name,seq)` for the AUTOINCREMENT table; it is not a fifth application table. There are no foreign keys or explicit actor/space columns in these four tables. Event payload `data` is JSON text by code contract, not a declared JSON column. `meta` holds string-keyed epoch/restore metadata (epoch, epoch_head, highwater; schema.rs:159–190); it is not a membership table.

`writer.rs:148–240` atomically appends event batches and request dedupe/receipt rows, rolls back duplicate idempotency conflicts, and broadcasts only after transaction success. Ordinary idempotency results expire; permanent publication receipts do not (`writer.rs:124–132`, `tests.rs:221`). `commit_once` upgrades to FULL for its transaction and returns to NORMAL afterward; ordinary journal writes retain NORMAL. History durability lives elsewhere. The auth **note_journal** is the canonical per-owner/per-note sharing admission stream; the per-person **journal** receives ordered events/publications. They are different sequence domains with different retention/reset behavior, not duplicate schemas to collapse without a protocol review.

The space port must make the journal domain explicit and use signed scope-bound cursors, preserving account attribution even if the old journal column set only carried installation/request IDs. Adding a space_id column everywhere is not the only implementation option: a validated store binding can partition a whole DB, while protected account events require their own domain. Architecture already settles **space partitioning of content**; storage binding, wire aliases and account-event partition details remain implementation contracts.

## 5. Person owns a Home: assumption inventory and adaptation options

This inventory covers concrete path constructors, authority fast paths and scope-bearing consumers found by git searches, not just occurrences of the word Home. Related call sites are grouped so each dependency has a migration consequence. Appendix B records literal production path matches; types/fast paths below explain assumptions that path searches miss. Tests/fixtures also encode the old boundary and must be converted into multi-account/multi-space cases, not simply renamed.

| Existing assumption / evidence at dev | Required distinction and options (none selected) | Cost and effect on existing disk data |
|---|---|---|
| Setup user UUID selects a new Home; create prints its path (`auth/setup.rs:160,397`; `calternal/main.rs:284`) | Account authentication must not implicitly select content. Options: bind each old Home UUID to that account's initial space, or assign a fresh space UUID with a verified explicit legacy binding/rekey. | Reuse UUID/binding preserves files and relative links in place but requires typed account/space IDs despite equal values. Fresh UUID requires aliases and either persistent root binding or offline verified directory/state/witness rekey; must not duplicate canonical bytes or reset identity/history. This changes ownership mapping, not the accepted directory shape/tier layout. |
| Caller.user is “whose Home” (`core/action.rs:18–27`); server Identity returns user + installation (`server/auth.rs:16–62`) | Always carry actor account separately from owning space and selected resource grant. Existing-account multi-space operations need explicit scope. | Type/protocol changes propagate to every action, recorder, room and DAV request. Existing actor IDs stay historical authors; do not rewrite all history authors to a space UUID. |
| Engine runtime map, inflight retries, sync APIs keyed by user (`engine/runtime.rs:43–81`; `engine/lib.rs:223–311`) | Content runtime/journal per space is settled. Options for personal event transport: separate account stream or multiplex explicitly typed account/space streams behind one connection. | Separate streams simplify access domains but add resume cursors; multiplex needs strict filtering/fencing. Both must retain old cursor expiry/reset and prevent replay disclosure. Existing per-person index.db cannot automatically become both shareable content and private account history. |
| Server Signed extraction and journal/live stream choose the authenticated person (`server/api.rs:18–29,94–104`; `server/live.rs:106–114`); room membership tracks string room IDs (`server/rooms.rs:99–110`) | Separate authenticated account from selected authorized journal and canonical resource owner; retain credential scope and live revalidation through reconnect/delivery. | Explicit space streams need space cursors and membership checks; typed multiplexing needs independent account/space resume and purge fences. Room IDs must not collide across owner spaces. Existing clients need aliases or a protocol reset, not a silent reinterpretation of their old user cursor. |
| Store signs a user directory's journal and derives K/users/<basename> (`store/lib.rs:196–199,625`) | Scope-bound keys/epochs distinguish account and space. Options: explicit mapped reuse of old domains or issue fresh domains with forced full authorized reset. | Reuse requires proofs that no account-private event becomes visible; fresh domains force client resync and preserve aliases/receipts separately. Existing witness/key must be kept for old-domain validation or deliberately retired offline, never silently reused for another owner. |
| Home.user, Notes homes/lists maps and state paths, discovery union (`notes/home.rs:125`; `notes/lib.rs:112,231,298,362–374`; `notes/scan.rs:464`) | Space owns rooms, namespace, metadata/history/jobs/watcher state; account remains author and access subject. | Retype the complete Home service, not just paths. A member in two spaces must never reuse the wrong Home cache; two members in one space must share the same canonical room. Existing pending writes/jobs/Trash stay with their owning content, not the enrolling person's future memberships. |
| Owner session fast paths and typed resource/history defaults (`notes/home.rs:231,254,1019`; `auth/sharing.rs:374,1771`; `core/sharing.rs:143–161,405–426`) | Replace account==owner authority with current scoped membership/resource access and credential intersection. Options: direct authoritative lookup each admission or a versioned revocable authority cache. | Lookup may cost DB work; cache needs bounded invalidation and commit/delivery rechecks. Neither may keep a removed member authorized. No file relocation needed; persisted old owner references need mapping. |
| Sharing owner_id references users; targets only person_id, control state per recipient/install (`auth/db.rs:197–329`; `sharing.rs:2518`) | Owner becomes space; targets include people and spaces; hide state and installations remain account-private. Options: explicit separate grant target tables or a typed target record with enforced referential integrity. | Separate tables cost joins/migrations; typed targets cost validation and invariant complexity. Both need effective role/grant intersection, immediate revocation fences and one canonical editable resource. Shared references never cause another Home copy or second quota charge. |
| Groups are local/OIDC with manager bit; directory visibility comes from co-membership (`auth/roles.rs:46`; `groups.rs:52`; `group_list.rs:7`) | Groups can remain server directory groups, be scoped to a space, or be explicit sources of space grants; no document makes them automatically spaces. | Preserve old groups and provenance until an explicit mapping is selected. Automatic conversion could widen access to all existing/future space content; dropping them loses invitation/visibility semantics. Provider removal must revoke effective grants without losing unrelated direct grants. |
| users.role has deployment capabilities; first person admin, last people manager protected (`auth/db.rs:62–87`; `roles.rs:65`) | Distinct deployment administration, space role and ownership. Options for owner representation: ownership relation alongside role, or an explicitly constrained owner membership attribute. | Both require transactional transfer-before-leave and last-owner checks. Manager alone is insufficient. Legacy admin must not become a content reader of every imported Home. Schema and recovery records migrate; user-visible files need no rewrite. |
| Invite creates a new user, assigns person quota/groups, then Home (`invite_redeem.rs:82–120`; `setup.rs:324`) | Options: separate admission and space-membership links, or one typed envelope that can carry both after explicit authorization. | Separate flows add onboarding steps; combined links require atomic multi-purpose validation and correct existing-account acceptance. Old unused invites cannot silently acquire grants to newly shared spaces. Existing encrypted tokens/key must survive, or be explicitly revoked and replaced with generic refusals preserved. |
| Quota users.quota_bytes; Usage first directory component; upload locks keyed UserId (`auth/quota.rs:58–101`; `fsio/usage.rs:60,97–125`) | Owning-space charging is settled; host needs canonical size/reservation and scope-aware accounting. Existing personal limit must be mapped to initial owning space or explicitly replaced by an approved plan allocation. | No discount from reflinks; moving between tiers is one logical file. Shared members must contend on one reservation. A recount must not count staging duplicates or charge a shared reference. Existing files/history remain intact above quota; notes must not be lost. |
| Attachments staged/indexed/served by user; sweep pairs state user dir with data user dir (`attachments/lib.rs:90,193,238–295`; `sweep.rs:26–31`) | Owning-space paths/index/placement plus actor attribution and scoped download resolution. | Keep Photos/Documents conventions and relative links; convert pending marker ownership before sweep or preserve legacy binding so restart does not delete another scope's temp file. Existing same-Home dedupe hints stay scoped. |
| Prefs and backdrops treated as person Home data (`prefs/store.rs:158–169`; `backdrops/uploads.rs:85–113`) | Personal profile storage is settled; per-setting classification and legacy split still required. Options: migrate recognized personal keys and quarantine unknown keys privately, or reviewed per-plugin setting migration before sharing. | Quarantine preserves privacy but may defer behavior; per-plugin migration costs more work. Copying the whole settings file into the new shareable space leaks account-local data. Backdrop reuse may retain an account-private original with authorized reference or make a charged independent space copy. |
| Search index/provider, Contacts raw/history, CardDAV book and Tasks order keyed user (`search/lib.rs:150`; `contacts-actions/write.rs:156`; `carddav/book.rs:28`; `tasks/order.rs:4`) | Space owns content index/order/protocol view; account owns external bindings. | Rebuild derived indexes by owning space after consistent mapping; preserve raw cards, immutable history and manual ranks. A multi-space personal search can aggregate authorized providers, not scan another space's cache. DAV credentials need selected targets, not universal account-home access. |
| Pairing/legacy checks enumerate users/<uuid>/notes/history.db; fixtures/importers construct that tree (`storage.rs:243–250`; `fixtures/lib.rs:106`; `migrate-v0/src/lib.rs:60`) | Validate bound Homes/spaces and account-private state separately using the existing layout. | Offline importer must move/copy **matching current** auth/history/receipts/companions, preserve IDs/relative names/modes and inventory, then bind ownership. Fixtures must include one actor in two spaces, two actors in one, and inaccessible shared references. Existing data is never adopted from filenames alone. |
| Socket/recovery commands operate deployment-wide; original create/recover opens global auth (`admin.rs:74`; `auth/setup.rs:79,171`) | Admin identity recovery remains account-level; space ownership never transfers automatically. | Keep protected local/admin paths and audit; require identity verification/compromised-session treatment. No need to move content for recovering an account. Global operator access is outside the agreed privacy threat model, not an implicit app browsing capability. |

**Existing data migration is not authorized by this review.** Either mapping strategy must preserve UUID/address aliases, note IDs/lineages, current and pending file bytes, original history, request/publication receipts, share/grant chronology, invite-key reachability, settings privacy, Trash and quota. A many-to-one merge of old Homes into one shared space would introduce path collisions, history/cursor merges and ownership changes beyond a small compatibility port; it requires explicit user intent, unlike a one-old-Home-to-one-initial-space mapping. Later content transfer is already a distinct authorized operation and must not be simulated as granting another membership.

## 6. FO1 security-before-ack audit

The accepted rule is unusually strong: security changes **and namespace intents** must be durably represented in a protected Home recovery snapshot before acknowledgement, while ordinary edit acknowledgements wait for SSD history and allow up to one second for durable Home placement. These are distinct deadlines. FO1 requires current identities, memberships, permissions, quotas, preferences, necessary keys and unfinished-operation recovery; expected installation UUID pinned outside lost state; validated root roles and complete versioned snapshots; rebuilding empty state only after taking every lock and validating proofs (`ADR 0062:404–458`). Ordinary export excludes credentials (rewrite inheritance audit); protected disaster recovery is a separate artifact with restricted access/modes.

| Examined path | What code actually does | FO1 implication |
|---|---|---|
| Auth connection durability (`auth/db.rs:413–452`) | WAL/FULL/FK and committed migrations; writes use the auth DB worker. | Full state durability is valuable, but a state-only WAL commit is not a Home recovery representation. |
| Enrollment (`auth/setup.rs:324–399`) | Consume token/create user/passkey/installation in one transaction, commit at :393, create Home directory at :397, return. | No Home security snapshot before ack. A crash can leave durable account state with no created Home; mkdir is not a protected versioned recovery record. |
| Quota (`auth/quota.rs:30–47`) | IMMEDIATE transaction checks people.manage, updates users.quota_bytes, commits, returns event. | Home-only restore cannot reconstruct this new quota from these writes. |
| Groups (`auth/groups.rs:157–253`) | Auth transactions update group/membership rows and return events after commit. | No Home materialization barrier in these functions; copying generic engine journal events does not prove all protected recovery state is represented. |
| Passkey/session management (`auth/passkeys.rs:201–233`; session revoke paths :320 onward) | Credential/installation state and in-memory invalidation live in auth owner; account events may later reach index journal. | Auth loss loses durable identity/security state. Neither account events nor key creation establish a complete snapshot or exclusion from tenant content APIs. |
| Share revocation (`auth/sharing.rs:2792–2828`) | Sequence event + revoked grant + recipient purge control commit to auth.db, then wake fanout/evict. | Protects online ordering and retry behavior, but no Home snapshot barrier is present. Home-only reconstruction cannot safely infer the current revocation from Markdown. |
| Invite key (`auth/invite_key.rs:12–64`) | Atomically publish private key in S/system, refuse missing key when rows exist. | Correct key custody on S; no protected Home recovery copy shown. Putting it in ordinary shared `.calternal/` would violate account/tenant separation. |
| Generic recoverable file exchange (`fsio/swap.rs:102–119,200`) | Adjacent durable intent can restore displaced settings bytes across restart. | Partial useful primitive; it is not a complete installation-bound auth snapshot or global FO1 coordinator. Do not claim no recovery intents exist anywhere. |
| Store publication (`store/writer.rs:65–95`; tests.rs:221) | FULL permanent receipt transaction can publish recipient journal batch once. | Prevents fanout duplication; not durable Home recovery metadata and not all-store atomic commit. |
| Startup (`calternal/storage.rs:103,119,178–189`) | Requires matching markers and nonempty auth.db; refuses creating fresh stores when missing. | Directly preserves original two-root contract and directly lacks FO1's validated empty-state rebuild. Keep this refusal until the stronger proof exists. |
| Contacts (`contacts-actions/cards.rs:31`; contacts/src/companion/mod.rs:1) | Current state-root raw cards and pure companion-format rules exist. | Pure companion tests do not establish materialized original cards or global identity recovery from Home. Preserve opaque properties/photos in eventual file recovery. |

**Conclusion verified from these code paths:** FO1's security-before-ack requirement is **not implemented as a complete foundation contract on origin/dev**. There is no evidence here for an installation-bound protected Home auth snapshot, root-role manifest, externally pinned UUID or empty-state auth reconstruction. A git grep for FO1/recovery-snapshot/`home-materialization-1s` in crates found companion-format references, not that global pipeline; this negative search supports, but does not substitute for, tracing the writes above. `docs/storage.md:24–30,45–49` accurately documents current auth/history non-rebuildability, whereas ADR FO1 explicitly labels the stronger behavior an implementation requirement (:451).

Port consequence: retain state-first security durability and online revoke checks, then add versioned protected recovery publication/intents and crash-safe reconciliation before acknowledging a security/namespace change. Publishing a snapshot before SQLite commit needs recoverable intent/completion rules to avoid restoring an uncommitted privilege grant; publishing after commit but before acknowledgement still needs crash handling to avoid lost revocations in a state-loss scenario. Neither ordering alone solves the two-store problem. The exact encoding/coordination is an engineering contract to prove, not a product vote to weaken FO1. No request to replace this with coordinated-backup-only recovery is made.

## 7. Proposed small port sequence (each independently testable)

All steps below use disposable fixtures and rusqlite; no real-data cutover is implied. Tests named are to port with the owning component and run in the rewrite when implemented, **not runs claimed by this document**. Feature exposure stays disabled where a required safety proof is absent. The sequence separates mechanisms so a failure has a bounded owner; it does not defer inherited launch requirements by implication.

| Step | Bounded change and independent acceptance | Existing tests to bring with it |
|---|---|---|
| 1. Filesystem primitives | Bring fsio guards, mode-aware atomic write, exclusive existing-root locks, no-follow opens, durable directory creation and platform-safe failures; verify outside replacements survive and no secret is briefly public. | fsio/tests.rs:22,42,59,73,107,150,223,258,390,582,623,708,731; beneath/swap inline cases. |
| 2. Root opening guard | Reuse two-root startup refusal fixtures and extend them to the fixed three-role target with explicit setup only; validate pinned identity/roles before creating stores. No Home data port needed. | storage_startup.rs:201,221,248,293,321,346,522,536,579; add swapped-role/symlink/missing-mount/all-lock tests from ADR :332–341. Old empty-state refusal test remains until FO1 step supplies validated recovery. |
| 3. Auth DB worker and exact legacy schema | Import rusqlite worker/migrations without enabling login endpoints; verify opening legacy fixtures is idempotent and failed migration rolls back without changing identity. Keep full schema, including sharing tables. | auth/tests/migration.rs:7; security_regressions.rs:424; db inline queue tests; reliability_review.rs:805. |
| 4. Account auth/session core | Port token/passkey/ceremony/session state, trusted origin/cookie rules and async auth adapter; first exercise synthetic identities and revalidation, then trusted endpoints. Account private data is never returned by space operations. | setup/create/login, host_cookies:8,53,110; passkeys:43,209,312; isolation:50,106,154; abuse/reliability prompt/forwarding tests. Add account in two spaces without any authority cached in session. OIDC linking needs new conformance cases because old code does not supply it. |
| 5. Space directory and owner mapping | Implement selected Q1 mapping using the reused directory shape; persistent membership roles, transfer-before-leave, current checks at dispatch/commit/replay/delivery. Recheck the same account in multiple spaces and multiple accounts in one space. | Rewrite authz_matrix; old engine/tests.rs:381, auth/security_regressions.rs:276,325 and calternal isolation/grants cases, adapted so admin and Manager are distinct. Add last-owner/transfer race and membership removal during a running action. |
| 6. Journal/store domain | Port ordered single-writer store, receipts, signed tokens, restore epochs and bounded idle release under explicit space/account bindings. Prove one member cannot retrieve another member's receipt and a cursor from another domain is invalid. | store/tests.rs:19,39,72,103,119,169,185,221; restore.rs:111,174,257,317,354,386,409,480,517,559,741,826,850 and restore_failures. engine/tests.rs:626 and engine restore tests. |
| 7. FO1 security coordinator | Add protected versioned account/space recovery records and unfinished security/namespace intents with no-ack-before-recovery durability. Test every crash point through grant/revoke, membership/owner transfer, key/session recovery, quotas and invites; rebuild only from fully validated Home, preserve corrupt nonempty state. | Existing settings recover_exact and storage rejection tests; auth invite/revoke/last-key tests; store restore/publication receipt tests. **New** FO1 state-loss, partial-snapshot, wrong UUID/roles, privilege non-resurrection and repeated-recovery-crash tests are mandatory; none of those suites alone is an existing end-to-end FO1 pass. |
| 8. Invites, groups and quota | Port one-winner invite cryptography and selected typed invitation/group mapping; owning-space admission and per-space placement reservations across users. Enable only after security coordinator can record their changes. | invites:19,141,250,295,453,548,592; security_regressions:96,222,236,367,521,561,597; groups:7; quota_counter:8,35; reliability_review:254,349,584,720,779; round3 slow-upload cleanup. Add existing-account acceptance and concurrent members spending one space quota. |
| 9. Canonical Home/history and namespace | Retype Home caches/rooms/history/Trash to space; keep actor IDs and old aliases; implement v3 current namespace map/selected-root and generation contracts. Separate app format engines from protected writes. | Notes index/history/restart_listing/kill9 suites; calternal notes_crash and notes_crash_typing; owner delivery tests adapted to role/grant checks. ADR :296–313,343–367 adds two-root folder/copy/open-inode/state-loss tests; v3 cache/tag/watcher gates need new fixtures. |
| 10. Sharing and attachments | Port sequenced resource grants, fanout/controls/reset/eviction with account-or-space targets; no canonical copy. Port bounded upload placement/sweep/download against owning-space roots. | Auth sharing inline suite :2876; calternal isolation/grants (including no-owner default :152,364); store publication_receipt :221; attachments HTTP/sniff/index_flush and attachments_crash; app_passwords scope/revoke/flood/odd-path tests. Add target-space member removal, share revocation during delivery and no cross-space dedupe signal. |
| 11. Profile/app-state split and migration qualification | Reviewed migration of personal prefs vs shared defaults, raw cards/history/manual order, derived search/DAV caches; prove both legacy-root layouts and interrupted offline migration preserve inventory and rollback. Independently validate full tiering/FO1 and performance before real data. | Prefs store/external-edit/recovery suites; contacts-actions correctness/kill9/review, carddav journal, tasks order, search rebuild; storage_startup:386 migration/rollback and calternal isolation split-state test :835. Bring v5 split-tree race/crash fixtures as additional cases, not accepted implementation by ancestry. Run ADR and v3 full gates only under current allocated/locked hosts; this lane runs none. |

## 8. Questions for the user

Only choices not settled by the supplied documents/request are listed. Implementation details such as SQL spelling, worker structure, recovery record encoding and test decomposition belong to an engineering contract; they are not permission questions. Physical tier/root roles, rusqlite, one space concept, roles, private account profile, canonical charging, protected recovery and the FO1 maximum are already settled.

1. **Legacy Home identity mapping:** should an old Home's UUID be retained as its initial space's storage identity, or should import allocate a fresh space UUID? Retaining a validated binding minimizes file moves and old-link churn but requires strong type separation from the equal-valued account ID. A fresh UUID requires explicit aliases and persistent legacy-root binding or verified offline rekey of content/state/keys/witness references; historical actor account IDs must remain unchanged. Neither option changes the accepted directory/tier design. Separately, `docs/morning-decisions.md` already asks whether current-data import must be in the first daily-use build; this review does not resolve that timing.
2. **Space invitation versus server admission:** use separate link types/flows, or one explicitly typed link that can admit a new account and join a specified space? Separate links simplify scope/revocation and existing-account acceptance but add onboarding steps. A combined link saves steps but needs atomic validation of both authorities and a rule for existing accounts; old account-creation invites must not silently widen access.
3. **Existing groups and people discovery:** retain directory-only groups with explicit independent space memberships, allow explicit group-to-space grants, or migrate selected groups into spaces after review? Directory-only preserves provenance but duplicates membership management; group grants simplify administration but make provider/group removal security-critical; conversion risks exposing all existing/future content and needs explicit mapping/collision review. The old group manager flag is not a space role, and old shared-group visibility is not a settled multi-space directory policy.
4. **Legacy mixed settings and uploaded backdrops:** preserve unclassified settings privately until reviewed, or require each app's settings migration before importing/sharing its space? Private quarantine avoids accidental disclosure and retains bytes but may temporarily lose app behavior; mandatory per-app classification delays import. For a backdrop used as a shared default, an authorized reference preserves one original but depends on its private owner's lifecycle; an independent space copy is portable to that space and consumes its quota. Personal layout privacy itself is not optional.
5. **Quota breakdown beyond canonical live content:** what allowance/budget split applies to retained history, account-private bytes and durable runtime/provider-session records, with the existing counted Trash/backdrop policy retained, and how should an imported person's quota map into initial space plan limits? One inclusive allowance is easy to explain but retained history/runtime growth can prevent new uploads; separate visible budgets reduce that coupling but need multiple meters and clear exhaustion behavior. Keep accepted retention, counted Trash/backdrops, logical-size charging and note-save protection either way; physical reflink savings never become a quota discount. The existing source policy is not a complete multi-space plan allocation.
6. **Administrator-assisted recovery procedure:** use one verified administrator or require a second administrator for a high-risk identity reset, and which independent notification channel is available when sign-in methods are lost? One administrator suits small self-hosts but concentrates trust; dual control reduces unilateral mistakes but cannot work unchanged on single-admin installations. A verified alternate channel helps notify the account owner but adds enrollment/maintenance obligations. The settled policy already permits protected admin-assisted recovery of the same account; this asks about verification/notification/audit procedure, not whether administrators may browse content or transfer spaces.

## Appendix A. Verbatim auth migrations at origin/dev

The compact effective schema above is backed by the exact migration SQL below, including FK actions, partial indexes, checks, seed data and triggers. Apply in numeric order for the v15 effective schema; obsolete host_cookie and scope triggers are deliberately visible as migration history, not asserted to remain in the final schema. Source: `crates/auth/src/db.rs:18–332`.

### Migration 1

```sql
CREATE TABLE users (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    username    TEXT NOT NULL UNIQUE COLLATE NOCASE,
    created_at  INTEGER NOT NULL
);
CREATE TABLE passkeys (
    cred_id      BLOB PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id),
    passkey      TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    last_used_at INTEGER
);
CREATE INDEX passkeys_user ON passkeys(user_id);
CREATE TABLE setup_tokens (
    hash        BLOB PRIMARY KEY,
    purpose     TEXT NOT NULL CHECK (purpose IN ('first-owner', 'recover')),
    user_id     TEXT REFERENCES users(id),
    created_at  INTEGER NOT NULL,
    expires_at  INTEGER NOT NULL,
    used_at     INTEGER
);
CREATE TABLE installations (
    id           TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id),
    kind         TEXT NOT NULL CHECK (kind IN ('browser', 'native', 'cli', 'agent', 'dav')),
    name         TEXT NOT NULL,
    token_hash   BLOB NOT NULL UNIQUE,
    created_at   INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    expires_at   INTEGER,
    revoked_at   INTEGER
);
CREATE INDEX installations_user ON installations(user_id);
```

### Migration 2

```sql
ALTER TABLE passkeys ADD COLUMN name TEXT NOT NULL DEFAULT 'Passkey';
```

### Migration 3

```sql
CREATE TABLE roles (name TEXT PRIMARY KEY);
CREATE TABLE role_capabilities (
    role TEXT NOT NULL REFERENCES roles(name),
    capability TEXT NOT NULL,
    PRIMARY KEY (role, capability)
);
INSERT INTO roles VALUES ('admin'), ('member');
INSERT INTO role_capabilities VALUES
    ('admin','people.invite'), ('admin','people.manage'),
    ('admin','groups.manage'), ('admin','server.settings');
-- SQLite cannot add a REFERENCES column with a default to a populated table.
-- Keep users in place and enforce that reference with triggers instead.
ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'member';
CREATE TRIGGER user_role_insert BEFORE INSERT ON users
WHEN NOT EXISTS (SELECT 1 FROM roles WHERE name=NEW.role)
BEGIN SELECT RAISE(ABORT,'unknown role'); END;
CREATE TRIGGER user_role_update BEFORE UPDATE OF role ON users
WHEN NOT EXISTS (SELECT 1 FROM roles WHERE name=NEW.role)
BEGIN SELECT RAISE(ABORT,'unknown role'); END;
CREATE TRIGGER role_in_use BEFORE DELETE ON roles
WHEN EXISTS (SELECT 1 FROM users WHERE role=OLD.name)
BEGIN SELECT RAISE(ABORT,'role in use'); END;
UPDATE users SET role='admin' WHERE id=(SELECT id FROM users ORDER BY created_at,rowid LIMIT 1);
CREATE TRIGGER first_person_admin AFTER INSERT ON users
WHEN (SELECT count(*) FROM users)=1
BEGIN UPDATE users SET role='admin' WHERE id=NEW.id; END;
```

### Migration 4

```sql
ALTER TABLE users ADD COLUMN quota_bytes INTEGER CHECK (quota_bytes IS NULL OR quota_bytes >= 0);
CREATE TABLE groups (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'local' CHECK (source IN ('local','oidc'))
);
CREATE TABLE group_members (
    group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id),
    manager INTEGER NOT NULL DEFAULT 0 CHECK (manager IN (0,1)),
    PRIMARY KEY (group_id,user_id)
);
CREATE INDEX group_people ON group_members(user_id,group_id);
CREATE TABLE invites (
    id TEXT PRIMARY KEY,
    created_by TEXT NOT NULL REFERENCES users(id),
    hash BLOB NOT NULL UNIQUE,
    secret BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    used_at INTEGER,
    revoked_at INTEGER,
    quota_bytes INTEGER CHECK (quota_bytes IS NULL OR quota_bytes >= 0),
    plan TEXT
);
CREATE TABLE invite_groups (
    invite_id TEXT NOT NULL REFERENCES invites(id) ON DELETE CASCADE,
    group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    PRIMARY KEY (invite_id,group_id)
);
```

### Migration 5

```sql
ALTER TABLE invites ADD COLUMN lookup_version INTEGER NOT NULL DEFAULT 0;
```

### Migration 6

```sql
ALTER TABLE installations ADD COLUMN host_cookie INTEGER NOT NULL DEFAULT 0;
```

### Migration 7

```sql
ALTER TABLE installations DROP COLUMN host_cookie;
```

### Migration 8

```sql
ALTER TABLE installations ADD COLUMN scopes TEXT NOT NULL DEFAULT '';
CREATE TRIGGER installation_scopes_insert BEFORE INSERT ON installations
WHEN (NEW.kind = 'dav') <> (NEW.scopes <> '')
BEGIN SELECT RAISE(ABORT,'dav installations need scopes, others must have none'); END;
CREATE TRIGGER installation_scopes_update BEFORE UPDATE OF kind, scopes ON installations
WHEN (NEW.kind = 'dav') <> (NEW.scopes <> '')
BEGIN SELECT RAISE(ABORT,'dav installations need scopes, others must have none'); END;
```

### Migration 9

```sql
ALTER TABLE installations ADD COLUMN assertion_at INTEGER;
DROP TRIGGER IF EXISTS installation_scopes_insert;
DROP TRIGGER IF EXISTS installation_scopes_update;
CREATE TRIGGER installation_scopes_insert BEFORE INSERT ON installations
WHEN (NEW.kind = 'dav') <> (NEW.scopes <> '')
  OR NEW.scopes NOT IN ('', 'contacts', 'calendar', 'calendar,contacts')
BEGIN SELECT RAISE(ABORT,'dav installations need known scopes, others must have none'); END;
CREATE TRIGGER installation_scopes_update BEFORE UPDATE OF kind, scopes ON installations
WHEN (NEW.kind = 'dav') <> (NEW.scopes <> '')
  OR NEW.scopes NOT IN ('', 'contacts', 'calendar', 'calendar,contacts')
BEGIN SELECT RAISE(ABORT,'dav installations need known scopes, others must have none'); END;
```

### Migration 10

```sql
ALTER TABLE installations ADD COLUMN registration_session INTEGER NOT NULL DEFAULT 0;
```

### Migration 11

```sql
DROP TRIGGER IF EXISTS installation_scopes_insert;
DROP TRIGGER IF EXISTS installation_scopes_update;
CREATE TRIGGER installation_scopes_insert BEFORE INSERT ON installations
WHEN (NEW.kind <> 'dav' AND NEW.scopes <> '')
  OR NEW.scopes NOT IN ('', 'contacts', 'calendar', 'calendar,contacts')
BEGIN SELECT RAISE(ABORT,'dav installations need known scopes, others must have none'); END;
CREATE TRIGGER installation_scopes_update BEFORE UPDATE OF kind, scopes ON installations
WHEN (NEW.kind <> 'dav' AND NEW.scopes <> '')
  OR NEW.scopes NOT IN ('', 'contacts', 'calendar', 'calendar,contacts')
BEGIN SELECT RAISE(ABORT,'dav installations need known scopes, others must have none'); END;

CREATE TABLE installation_notes_scopes (
    installation_id TEXT NOT NULL REFERENCES installations(id) ON DELETE CASCADE,
    scope TEXT NOT NULL CHECK (scope IN ('notes.read','notes.write','notes.share')),
    PRIMARY KEY (installation_id, scope)
) STRICT;
CREATE TRIGGER installation_notes_scopes_insert BEFORE INSERT ON installation_notes_scopes
WHEN (SELECT kind FROM installations WHERE id=NEW.installation_id) <> 'dav'
BEGIN SELECT RAISE(ABORT,'only app passwords may have Notes scopes'); END;

CREATE TABLE note_shares (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id),
    note_id TEXT NOT NULL,
    last_note_seq INTEGER NOT NULL DEFAULT 0 CHECK (last_note_seq >= 0),
    created_at INTEGER NOT NULL,
    creator_installation TEXT REFERENCES installations(id),
    UNIQUE (owner_id, note_id)
) STRICT;
CREATE TABLE share_grants (
    id TEXT PRIMARY KEY,
    share_id TEXT NOT NULL REFERENCES note_shares(id) ON DELETE CASCADE,
    person_id TEXT NOT NULL REFERENCES users(id),
    permission TEXT NOT NULL CHECK (permission IN ('read','edit')),
    grant_start_seq INTEGER NOT NULL CHECK (grant_start_seq >= 0),
    revoked_at_seq INTEGER CHECK (revoked_at_seq IS NULL OR revoked_at_seq > grant_start_seq),
    created_by TEXT NOT NULL REFERENCES users(id),
    revoked_by TEXT REFERENCES users(id)
) STRICT;
CREATE UNIQUE INDEX share_grants_active_person
    ON share_grants(share_id, person_id) WHERE revoked_at_seq IS NULL;
CREATE INDEX share_grants_person ON share_grants(person_id, share_id);

CREATE TABLE note_journal (
    owner_id TEXT NOT NULL,
    note_id TEXT NOT NULL,
    note_seq INTEGER NOT NULL CHECK (note_seq > 0),
    event_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    item TEXT NOT NULL,
    data TEXT NOT NULL,
    actor_id TEXT NOT NULL REFERENCES users(id),
    installation_id TEXT REFERENCES installations(id),
    request_id TEXT,
    at_ms INTEGER NOT NULL,
    PRIMARY KEY (owner_id, note_id, note_seq),
    UNIQUE (owner_id, event_id),
    FOREIGN KEY (owner_id, note_id) REFERENCES note_shares(owner_id, note_id)
) STRICT;
CREATE TABLE share_fanout (
    owner_id TEXT NOT NULL,
    note_id TEXT NOT NULL,
    note_seq INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (owner_id, note_id, note_seq),
    FOREIGN KEY (owner_id, note_id, note_seq)
        REFERENCES note_journal(owner_id, note_id, note_seq) ON DELETE CASCADE
) STRICT;
CREATE INDEX share_fanout_pending ON share_fanout(created_at, owner_id, note_id, note_seq);

CREATE TABLE recipient_control (
    id TEXT PRIMARY KEY,
    recipient_id TEXT NOT NULL REFERENCES users(id),
    share_id TEXT NOT NULL REFERENCES note_shares(id) ON DELETE CASCADE,
    note_seq INTEGER NOT NULL CHECK (note_seq >= 0),
    kind TEXT NOT NULL CHECK (kind IN ('purge','delete','baseline','loss-of-edit')),
    created_at INTEGER NOT NULL,
    UNIQUE (recipient_id, share_id, note_seq, kind)
) STRICT;
CREATE TABLE recipient_control_acks (
    control_id TEXT NOT NULL REFERENCES recipient_control(id) ON DELETE CASCADE,
    installation_id TEXT NOT NULL REFERENCES installations(id) ON DELETE CASCADE,
    acknowledged_at INTEGER NOT NULL,
    PRIMARY KEY (control_id, installation_id)
) STRICT;
CREATE TABLE recipient_share_state (
    recipient_id TEXT NOT NULL REFERENCES users(id),
    share_id TEXT NOT NULL REFERENCES note_shares(id) ON DELETE CASCADE,
    hidden INTEGER NOT NULL DEFAULT 0 CHECK (hidden IN (0,1)),
    PRIMARY KEY (recipient_id, share_id)
) STRICT;
```

### Migration 12

```sql
CREATE TABLE share_fanout_acks (
    owner_id TEXT NOT NULL,
    note_id TEXT NOT NULL,
    note_seq INTEGER NOT NULL,
    installation_id TEXT NOT NULL REFERENCES installations(id) ON DELETE CASCADE,
    acknowledged_at INTEGER NOT NULL,
    PRIMARY KEY (owner_id, note_id, note_seq, installation_id),
    FOREIGN KEY (owner_id, note_id, note_seq)
        REFERENCES share_fanout(owner_id, note_id, note_seq) ON DELETE CASCADE
) STRICT;
CREATE INDEX share_fanout_acks_installation
    ON share_fanout_acks(installation_id, owner_id, note_id, note_seq);
CREATE TABLE recipient_share_fences (
    recipient_id TEXT NOT NULL REFERENCES users(id),
    share_id TEXT NOT NULL REFERENCES note_shares(id) ON DELETE CASCADE,
    note_seq INTEGER NOT NULL CHECK (note_seq >= 0),
    PRIMARY KEY (recipient_id, share_id)
) STRICT;
CREATE TABLE sharing_installation_resets (
    installation_id TEXT PRIMARY KEY REFERENCES installations(id) ON DELETE CASCADE,
    required_at INTEGER NOT NULL
) STRICT;
CREATE INDEX recipient_control_note_order
    ON recipient_control(recipient_id, share_id, note_seq, id);
```

### Migration 13

```sql
CREATE TABLE sharing_reset_cursors (
    installation_id TEXT NOT NULL REFERENCES installations(id) ON DELETE CASCADE,
    owner_id TEXT NOT NULL REFERENCES users(id),
    note_id TEXT NOT NULL,
    note_seq INTEGER NOT NULL CHECK (note_seq >= 0),
    PRIMARY KEY (installation_id, owner_id, note_id)
) STRICT;
CREATE INDEX sharing_reset_cursors_note
    ON sharing_reset_cursors(owner_id, note_id, note_seq);
```

### Migration 14

```sql
ALTER TABLE note_shares ADD COLUMN deleted_at_seq INTEGER
    CHECK (deleted_at_seq IS NULL OR deleted_at_seq > 0);
```

### Migration 15

```sql
CREATE TABLE share_fanout_deliveries (
    owner_id TEXT NOT NULL,
    note_id TEXT NOT NULL,
    note_seq INTEGER NOT NULL,
    recipient_id TEXT NOT NULL REFERENCES users(id),
    delivered_at INTEGER NOT NULL,
    PRIMARY KEY (owner_id, note_id, note_seq, recipient_id),
    FOREIGN KEY (owner_id, note_id, note_seq)
        REFERENCES share_fanout(owner_id, note_id, note_seq) ON DELETE CASCADE
) STRICT;
CREATE INDEX share_fanout_deliveries_recipient
    ON share_fanout_deliveries(recipient_id, delivered_at);
```

## Appendix B. Literal production Home path inventory

This bounded inventory is a reproducible path-constructor/comment search, excluding separately named test/fixture/benchmark sources; it complements the typed scope and authority inventory in §5. It is evidence of concrete old paths, not a claim that grep alone proves every runtime behavior. Search patterns: `join("users")`, `users/<`, `users/{`, and `strip_prefix("users`. All lines below refer to pinned origin/dev.

```text
origin/dev:crates/attachments/src/index.rs:6://! where (ADR 0027). It lives at `users/<id>/attachments/index.db`. Losing
origin/dev:crates/attachments/src/lib.rs:237:    /// `data/users/<id>`.
origin/dev:crates/attachments/src/lib.rs:239:        self.data.join("users").join(user.to_string())
origin/dev:crates/attachments/src/lib.rs:242:    /// `data/users/<id>/attachments`: this plugin's own data.
origin/dev:crates/attachments/src/lib.rs:245:            .join("users")
origin/dev:crates/attachments/src/stage.rs:16://! to `users/<id>/attachments/pending/`. If the server dies mid-upload
origin/dev:crates/attachments/src/stage.rs:60:    /// The Home: `users/<id>`.
origin/dev:crates/attachments/src/stage.rs:62:    /// Crash markers: `users/<id>/attachments/pending`.
origin/dev:crates/attachments/src/sweep.rs:5://! `users/<id>/attachments/pending/`. [`sweep`] reads the markers, removes
origin/dev:crates/attachments/src/sweep.rs:26:    let Ok(users) = std::fs::read_dir(state.join("users")) else {
origin/dev:crates/attachments/src/sweep.rs:31:        let home = data.join("users").join(user.file_name());
origin/dev:crates/auth/src/setup.rs:161:    /// Their id: their Home is `<data>/users/<id>`.
origin/dev:crates/auth/src/setup.rs:397:    std::fs::create_dir_all(inner.data.join("users").join(user.to_string()))
origin/dev:crates/backdrops/src/uploads.rs:3://! They live in `users/<id>/backdrops/`, named by a hash of their content
origin/dev:crates/backdrops/src/uploads.rs:100:            .join("users")
origin/dev:crates/calternal/src/main.rs:284:                data.join("users").join(&c.id).display(),
origin/dev:crates/calternal/src/storage.rs:243:    match std::fs::read_dir(data.join("users")) {
origin/dev:crates/carddav/src/book.rs:28:        let state = state_root.join("users").join(user.to_string());
origin/dev:crates/carddav/src/cache.rs:1://! Disposable, per-user wire cache at `users/<uuid>/carddav.db`.
origin/dev:crates/carddav/src/cache.rs:102:    /// `user_state` is the state root's `users/<stable-user-id>` directory.
origin/dev:crates/carddav/src/lib.rs:10://! `users/<uuid>/carddav.db`. Reads also cross a replay barrier, so a lagged
origin/dev:crates/contacts-actions/src/cards.rs:7://! `<state>/users/<uuid>/contacts/raw/<calternal-id>.vcf` (ADR 0063, 0064),
origin/dev:crates/contacts-actions/src/cards.rs:30:    /// The copies under `user_state` (`<state>/users/<uuid>`).
origin/dev:crates/contacts-actions/src/write.rs:119:    /// `state/users/<uuid>/contacts/`. Lends the notes plugin the contacts
origin/dev:crates/contacts-actions/src/write.rs:156:        Cards::new(&self.state.join("users").join(user.to_string()))
origin/dev:crates/engine/src/lib.rs:123:    /// `data_dir/users/<id>/`.
origin/dev:crates/engine/src/runtime.rs:76:        let dir = self.root.join("users").join(user.to_string());
origin/dev:crates/fsio/src/usage.rs:33:/// that person's folder there (`<state>/users/<id>`).
origin/dev:crates/fsio/src/usage.rs:61:        let root = data.join("users");
origin/dev:crates/fsio/src/usage.rs:62:        let state_users = Some(state.join("users")).filter(|s| *s != root);
origin/dev:crates/migrate-v0/src/lib.rs:60:    /// The new Home folder (`<data>/users/<uuid>` or a staging folder).
origin/dev:crates/notes/src/home.rs:127:    /// `<data>/users/<uuid>/Notes`.
origin/dev:crates/notes/src/home.rs:129:    /// `<data>/users/<uuid>/Trash`: deleted notes, kept 30 days (see
origin/dev:crates/notes/src/home.rs:281:    /// Opens a Home under `user_dir` (`<data>/users/<uuid>`). Fast: loads
origin/dev:crates/notes/src/lib.rs:152:    /// Notes kept under `data/users/<uuid>/`.
origin/dev:crates/notes/src/lib.rs:231:    /// tasks' manual order. `<state>/users/<uuid>/<plugin>`.
origin/dev:crates/notes/src/lib.rs:362:    /// `<data>/users/<uuid>`.
origin/dev:crates/notes/src/lib.rs:364:        self.data.join("users").join(user.to_string())
origin/dev:crates/notes/src/lib.rs:368:    /// (`<state>/users/<uuid>`), for readers without an open Home.
origin/dev:crates/notes/src/lib.rs:374:        self.state.join("users").join(user.to_string())
origin/dev:crates/notes/src/scan.rs:464:            let dirs = [me.data.join("users"), me.state.join("users")];
origin/dev:crates/prefs/src/lib.rs:5://! `users/<id>/.calternal/settings.json` (ADR 0055; older homes have
origin/dev:crates/prefs/src/lib.rs:55:    /// Creates the plugin. Each user's file lives under `data/users/<id>/`.
origin/dev:crates/prefs/src/store.rs:51:/// `users/<id>/prefs.json` has them read from it until their first
origin/dev:crates/prefs/src/store.rs:149:    /// Creates a store for `data/users/<id>/`. Nothing is read yet.
origin/dev:crates/prefs/src/store.rs:159:        self.data.join("users").join(user.to_string())
origin/dev:crates/search/src/lib.rs:113:    /// Search over `providers`, with indexes under `data/users/<uuid>/`.
origin/dev:crates/search/src/lib.rs:150:                .join("users")
origin/dev:crates/search/src/schema.rs:1://! The search index on disk: `users/<uuid>/search/search.db`.
origin/dev:crates/store/src/lib.rs:198:            (Some(root), Some(user)) => Some(root.join("users").join(user)),
origin/dev:crates/tasks/src/order.rs:4://! It is app data, not derived from the files: `users/<id>/tasks/order.db`
```

## Appendix C. Direct production dependency inventory

For the adjacent owners in §2, this manifest inventory complements the subsystem coupling descriptions. Names are direct dependencies exactly as declared at the pinned revision, including target-specific dependencies; dev-dependencies are covered by the test-port rows. This is a dependency inventory, not a requirement to copy every product integration into the new host.

| Crate | Direct production dependencies | Manifest evidence |
|---|---|---|
| auth | `axum`, `base64`, `blake3`, `calternal-core`, `calternal-engine`, `calternal-fsio`, `calternal-server`, `getrandom`, `openssl`, `rusqlite`, `schemars`, `serde`, `serde_json`, `thiserror`, `tokio`, `tracing`, `unicode-casefold`, `unicode-general-category`, `unicode-normalization`, `unicode-security`, `url`, `uuid`, `webauthn-rs` | `crates/auth/Cargo.toml:8` |
| store | `blake3`, `calternal-core`, `calternal-fsio`, `rusqlite`, `serde_json`, `tokio`, `tracing`, `uuid` | `crates/store/Cargo.toml:8` |
| fsio | `arc-swap`, `rustix`, `tracing`, `unicode-normalization` | `crates/fsio/Cargo.toml:11` |
| calternal | `axum`, `calternal-attachments`, `calternal-auth`, `calternal-backdrops`, `calternal-contacts-actions`, `calternal-core`, `calternal-daily`, `calternal-engine`, `calternal-fsio`, `calternal-memory`, `calternal-migrate-v0`, `calternal-notes`, `calternal-plugin-hello`, `calternal-prefs`, `calternal-search`, `calternal-server`, `calternal-tasks`, `serde_json`, `tokio`, `tracing`, `tracing-subscriber` | `crates/calternal/Cargo.toml:8` |
| core | `schemars`, `serde`, `serde_json`, `thiserror`, `tokio`, `tracing`, `uuid` | `crates/core/Cargo.toml:8` |
| engine | `blake3`, `calternal-core`, `calternal-store`, `serde`, `serde_json`, `tokio`, `tracing` | `crates/engine/Cargo.toml:8` |
| server | `axum`, `calternal-core`, `calternal-engine`, `calternal-fsio`, `calternal-memory`, `calternal-store`, `http-body-util`, `serde`, `serde_json`, `tokio`, `tower-http`, `tracing`, `uuid` | `crates/server/Cargo.toml:8` |
| notes | `base64`, `blake3`, `calternal-core`, `calternal-fsio`, `calternal-markdown-view`, `calternal-memory`, `hashbrown`, `memchr`, `notify`, `pulldown-cmark`, `rusqlite`, `rustix`, `schemars`, `serde`, `serde_json`, `serde_yaml`, `sha2`, `similar`, `time`, `tokio`, `tracing`, `unicode-normalization`, `unicode-segmentation`, `uuid`, `yaml-rust2`, `yrs` | `crates/notes/Cargo.toml:8` |
| attachments | `axum`, `blake3`, `calternal-core`, `calternal-engine`, `calternal-fsio`, `calternal-server`, `http-body-util`, `imagesize`, `infer`, `percent-encoding`, `rusqlite`, `schemars`, `serde`, `serde_json`, `time`, `tokio`, `tokio-util`, `tracing`, `uuid` | `crates/attachments/Cargo.toml:8` |
| prefs | `calternal-core`, `calternal-fsio`, `schemars`, `serde`, `serde_json`, `tokio` | `crates/prefs/Cargo.toml:8` |
| backdrops | `axum`, `blake3`, `calternal-core`, `calternal-fsio`, `calternal-server`, `http-body-util`, `imagesize`, `serde`, `serde_json`, `tokio`, `tracing`, `ureq` | `crates/backdrops/Cargo.toml:8` |
| search | `calternal-core`, `calternal-engine`, `calternal-memory`, `rusqlite`, `schemars`, `serde`, `serde_json`, `time`, `tokio`, `tracing` | `crates/search/Cargo.toml:8` |
| contacts-actions | `blake3`, `calternal-contacts`, `calternal-core`, `calternal-fsio`, `calternal-notes`, `schemars`, `serde`, `serde_json`, `tokio`, `tracing`, `unicode-segmentation`, `uuid` | `crates/contacts-actions/Cargo.toml:8` |
| carddav | `axum`, `blake3`, `calternal-contacts`, `calternal-contacts-actions`, `calternal-core`, `calternal-engine`, `roxmltree`, `rusqlite`, `serde_json`, `tokio`, `uuid` | `crates/carddav/Cargo.toml:8` |
| tasks | `blake3`, `calternal-core`, `calternal-fsio`, `calternal-markdown-view`, `calternal-notes`, `pulldown-cmark`, `rusqlite`, `schemars`, `serde`, `serde_json`, `time`, `tokio`, `tracing` | `crates/tasks/Cargo.toml:8` |
