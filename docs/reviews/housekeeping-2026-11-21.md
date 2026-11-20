# Housekeeping report, 2026-11-21

Scope: the owner's request "close stale prs/issues, delete stale branches/worktrees". Rule for the night: delete only what is provably finished or empty, never force, list the rest. Protected branches, worktrees and issues were not touched. The other repository (`/home/kayg/Developer/calternal`) was not touched.

## Summary

| Item | Done |
|---|---|
| Worktrees removed | 20 (73 listed at the start; 3 more appeared during the run from other lanes) |
| Local branches deleted (`git branch -d`) | 46 |
| Remote branches deleted | 6 branches, 8 deletions (origin 5, github 3; list below) |
| Issues closed | 4 (#70, #77, #102, #104) |
| Issues commented "paused" | 7 (#92, #93, #94, #95, #96, #101, #105) |
| Build machine | 4 `node_modules` removed, about 0.8 GB |

No force was used anywhere. No builds were run. No agents were spawned.

## 1. Worktrees removed (clean, and branch or commit already in `origin/dev` or a protected branch)

`lane/sharing-2`, `lane/accounts`, `lane/accounts-protected-writes` (the three bb-owned ones belong to archived threads), `lane/admission`, `lane/automations-2`, `lane/codex-harness`, `lane/dev-fixes`, `lane/docs-regen`, `lane/live-rooms`, `lane/machine-origin-hotfix`, `lane/machine-ux`, `lane/onboarding-state`, `lane/plugin-dx`, `lane/shared-fetch`, `lane/upgrades` (dir `turn-meta`), `lane/turn-persist`, `lane/ui-threads`, `lane/v2-renderer-web`, and two detached review checkouts (`wake-review`, `window-review`, both ancestors of protected branches).

## 2. Local branches deleted (46, all ancestors of `origin/dev`, no worktree left)

`coord-stack`, `lane/accounts`, `lane/accounts-protected-writes`, `lane/admission`, `lane/automations`, `lane/automations-2`, `lane/automations-handoff`, `lane/codex-harness`, `lane/dev-fixes`, `lane/docs-regen`, `lane/installer-upgrade-fix`, `lane/installer-v2`, `lane/live-rooms`, `lane/live-rooms-2`, `lane/machine-origin-hotfix`, `lane/machine-ux`, `lane/night-6-plugin-gate`, `lane/onboarding-state`, `lane/perf-3`, `lane/plugin-dx`, `lane/plugin-fetch-submission`, `lane/plugin-wait-authoring`, `lane/proposal-review`, `lane/search-2`, `lane/setup-code`, `lane/shared-fetch`, `lane/shared-real-server-e2e`, `lane/sharing-2`, `lane/sharing-schema`, `lane/threads-ready`, `lane/threads-safe`, `lane/turn-persist`, `lane/ui-shell`, `lane/ui-shell-fix`, `lane/ui-shell-hold`, `lane/ui-shell-known`, `lane/ui-shell-native2`, `lane/ui-shell-pagekeys`, `lane/ui-shell-quiet`, `lane/ui-shell-wip`, `lane/ui-threads`, `lane/upgrades`, `lane/upgrades-2`, `lane/upgrades-3`, `lane/upgrades-4`, `lane/v2-renderer-web`.

Local branches kept because they are not in dev (28, with commits not in dev): `backup/admission-night5-8813592f`, `combined-ui`, `integrate/wake`, `integrate/window`, `lane/description-contract-ui`, `lane/installer`, `lane/machine-relay`, `lane/notes-block-ops`, `lane/perf-2`, `lane/plugin-api-v2`, `lane/plugin-fetch-trial`, `lane/plugin-waits`, `lane/receipt-deltas`, `lane/search-lookups`, `lane/search-postings`, `lane/space-machines`, `lane/threads-backend`, `lane/threads-backend-retired-8813592f`, `lane/threads-turn-retirement`, `lane/turn-meta`, `lane/ui-notes-bar`, `lane/ui-shell-churn`, `lane/ui-shell-focus-wip`, `lane/ui-shell-keyed-focus`, `lane/ui-shell-keyed-rows`, `lane/ui-shell-keyed-rows-wip`, `lane/v2-renderer-apple`, `wire/web-docs-gate`. Several of them are single-commit remnants whose remote twin was deleted below (`lane/space-machines`, `lane/threads-turn-retirement`, `lane/ui-shell-churn`); those three local copies are now also patch-equivalent to dev and can go on the owner's word. `lane/v2-renderer-apple` locally still carries the identifier history; it was never pushed anywhere by this lane.

## 3. Remote branches

Deleted (evidence in brackets):

| Branch | Removed from | Evidence |
|---|---|---|
| `lane/space-machines` | origin, github | (a) `git cherry` shows only `-` |
| `lane/threads-turn-retirement` | origin, github | (a) |
| `lane/ui-shell-churn` | origin | (a) |
| `lane/plugin-api-v2-rev13` | github | (b) merge-tree with dev equals dev's tree |
| `lane/server-inventory` | origin | (b) |
| `lane/v2-renderer-apple` | origin (not on github) | (b), and the owner's special case: its history holds an identifier that must never reach GitHub |

Kept: 29 unprotected branches that carry changes dev does not have, plus 22 protected refs. The table lists the 29. "Not in dev" is the count of `git cherry` `+` commits. Recommendations are for the owner; nothing here was deleted.

| Branch | Remotes | Last commit | Not in dev | What it seems to be | Recommendation |
|---|---|---|---|---|---|
| `hotfix/prod-web-hold` | both | 2026-11-18 | 1 | Web client: only 502/503/504 counts as outage; has a live `/tmp/coord-hotfix` worktree | ask (coordinator hotfix, may be pending) |
| `lane/api-v2-step0` | both | 2026-11-19 | 32 | Plugin API v2 step 0 spike (#78); has a worktree | keep until the primitives host settles |
| `lane/description-contract` | origin | 2026-11-19 | 58 | Description contract rev 11.x (native lane); has a worktree | keep (design history) |
| `lane/installer` | both | 2026-11-18 | 9 | Installer fixtures (#66) | ask |
| `lane/machine-relay` | both | 2026-11-18 | 18 | Thread sandbox preview relay (#68) | ask |
| `lane/mail` | both | 2026-11-18 | 48 | Mail plugin data layer (#69); has worktree `fonts-backend` | keep |
| `lane/notes-block-ops` | both | 2026-11-18 | 1 | WIP: Notes block operation wire shapes | ask, probably delete |
| `lane/notes-bytes` | both | 2026-11-18 | 1 | Preserve Markdown bytes when saving Notes; has a worktree | ask (may be unmerged real fix) |
| `lane/notes-room-parity` | both | 2026-11-18 | 1 | Room updates retain author tags (#61) | ask |
| `lane/notes-style` | both | 2026-11-18 | 3 | WIP style-sheet port (#73, paused) | keep (paused issue) |
| `lane/plans-interface` | both | 2026-11-18 | 20 | Plans interface (#56) | ask |
| `lane/plugin-api-v2-combined` | both | 2026-11-19 | 93 | Combined v2 contract drafts; worktree in `/tmp` | ask, likely superseded by v2 host |
| `lane/plugin-api-v2` | both | 2026-11-19 | 7 | Pins renderer handoff revision | ask, likely superseded |
| `lane/plugin-api-v2-rev14-draft` | both | 2026-11-20 | 39 | Rev 14 draft and host lane plan; has a worktree | keep (today's) |
| `lane/plugin-fetch-trial` | both | 2026-11-18 | 22 | Plugin fetch authoring (#58) | ask |
| `lane/plugin-waits` | both | 2026-11-18 | 2 | Focused plugin checks on the night-4 stack | delete after owner's word |
| `lane/receipt-deltas` | both | 2026-11-18 | 6 | v4 recovery reference regeneration | ask |
| `lane/space-runtime-measure` | both | 2026-11-19 | 3 | Always-on agent VM density; the commit cited by #77 | keep until the report is confirmed on dev (it is, see #77) then delete |
| `lane/threads-16mib` | both | 2026-11-18 | 33 | Threads backend 16 MiB append measurements (#53) | ask |
| `lane/threads-backend` | both | 2026-11-18 | 13 | Threads backend lane (#53) | ask |
| `lane/ui-notes-bar` | both | 2026-11-18 | 5 | Editor port status docs (#72) | ask |
| `lane/ui-shell-keyed-anchor` | origin | 2026-11-19 | 3 | WIP experiment, keyed VirtualList anchoring | delete after owner's word |
| `lane/ui-shell-keyed-focus` | origin | 2026-11-19 | 2 | WIP experiment, keyed focus | delete after owner's word |
| `lane/ui-shell-keyed-rows` | origin | 2026-11-19 | 1 | Experiment, rows keyed by item id | delete after owner's word |
| `lane/ui-shell-keyed-rows-wip` | origin | 2026-11-19 | 3 | WIP snapshot of the same | delete after owner's word |
| `lane/upgrades-5` | both | 2026-11-18 | 3 | Final fresh activation docs; the owner has since ruled "no upgrade proofs" | delete after owner's word |
| `lane/vm-measure` | origin | 2026-11-20 | 7 | Pre-started worker randomness measurements; has a worktree | keep (today's) |
| `lane/web-push-rate-limit` | both | 2026-11-18 | 2 | Push refusals classification (#64) | ask |
| `stack/night-8` | origin | 2026-11-18 | 18 | Integration stack merge of night 8 | delete after owner's word |

## 4. Worktrees kept

Protected (26): the main checkout excluded, these matched the protected patterns (design, v2-host, threads, window, ios, ui-shell/timing-gate, editor-chrome, mail-ui, ui-notes, plugin-memory, sdk-v2-authoring, space-vm-wake) or are this lane's own.

Unprotected and kept (these are not ancestors of dev, or are dirty):

| Worktree | Branch | State | Recommendation |
|---|---|---|---|
| `microvm` | `lane/microvm` | **dirty, 154 files**, branch in dev | ask: look at what is uncommitted, then remove |
| `perf-profile` | `lane/perf-profile` | **dirty, 14 files** | ask |
| `plugin-waits` | `lane/plugin-api-v2-step1` | **dirty, 9 files** | ask |
| `search` | `lane/search` | **dirty, 5 files**, branch in dev | ask |
| `plans-interface` (bb, archived thread) | `lane/plans-interface` | clean, 20 commits not in dev | ask |
| `api-v2-rev14-renderer`, `api-v2-step0`, `description-contract`, `fonts-backend` (`lane/mail`), `live-rooms-2` (`lane/notes-room-parity`), `perf-2`, `search-2` (`lane/notes-bytes`), `server-inventory`, `threads-backend`, `upgrades-2`, `vm-measure`, `ui-desc` (`rev5-renderer`) | various | clean, commits not in dev | keep until their branches are decided in the table above |
| `api-v2-review`, `rev14-review`, `native-review`, `proposal-review` | detached | clean, reviewed commits not in dev or any protected branch | ask; review checkouts, probably removable once the review is read |
| `/tmp/calternal-v2-rev6.bpVmMJ` | `lane/plugin-api-v2-combined` | clean, not in dev | ask (temporary directory, will vanish on reboot) |
| `/tmp/coord-deploy`, `/tmp/coord-stack`, `/tmp/coord-web` | detached | clean and in dev, but they are the coordinator's scratch checkouts | not removed on purpose: may be in use for a deploy; removable on the owner's word |
| `/tmp/coord-hotfix` | `hotfix/prod-web-hold` | clean, 1 commit not in dev | ask |

## 5. Forgejo issues

Closed (checklist complete and work on dev, or finished study as the owner said):

- #70 done, `afe91183` on dev.
- #77 done, report on dev (`docs/measurements/space-runtime-walls.md`).
- #102, #104 finished studies, commented with the document link. Both documents live only on protected branches (`lane/threads-split`, `lane/ios-keyboard-study`), not on dev yet, so those branches must stay until the documents are merged.

Commented "Paused 2026-11-20: the host is being redesigned as primitives only (#107)" and left open: #92, #93, #94, #95, #96, #101, #105.

**#75 was not closed**: its checklist is ticked, but the work lives on the protected `lane/ios-shell` (38 commits not in dev) and the lane is still reporting. Because it stayed open, none of its attachments (including captures of a third-party app, if any) were deleted. Recommendation: once the lane merges and the owner closes #75, delete those captures then. I did not test whether `fj` can delete attachments; the Forgejo web UI can.

Open issues needing the owner's word (everything else stays open untouched; 59 are open now). I did not close any of these because of open questions, unchecked boxes, or unmerged work:

| Issue | Observation | Recommendation |
|---|---|---|
| #20 backgrounds study | checklist done, doc on dev, but five owner questions unanswered | close after the owner answers or says defer |
| #34 dev end-to-end server | checklist done, `9f8ecac` on dev; "live browser owner setup remains" | close |
| #45 Quadlet deploy | checklist done, on dev; production cutover awaits the owner | keep until cutover |
| #103 three ways to split plugin memory | 0 of 5 ticked, but the lane (`plugin-memory`) has measured | keep, lane still protected |
| #73 notes style sheets, #72 editor chrome | explicitly PAUSED titles | keep |
| #5, #15, #17, #31, #33, #46, #47, #48, #50, #51, #52, #53, #54, #55, #56, #57, #58, #59, #60, #61, #62, #63, #64, #65, #66, #67, #68, #69, #71, #74, #76, #78, #81, #82, #83, #85, #88, #90, #91, #97, #98, #99, #100, #106, #107 | at least one unchecked box or no checklist; most predate the v2 host and primitives redesign | the owner should skim the older ones (#5 to #71) after the primitives decision and close those the redesign makes obsolete; I cannot prove they are finished |

Several early issues (#5, #15, #17, #31, #33) are four or more checklist items done with one open; they look like candidates to close once the owner confirms the last item.

## 6. Build machine `claude@calternal-dev-01-compute02`

Disk: 402 GB used of 492 GB, 71 GB free (86%). Nothing under `~/Developer/Worktrees` was touched, and no process was stopped.

Removed (`node_modules`, belonging to archived or deleted lanes with no running process): `lanes/accounts` (181 MB), `lanes/accounts-followup` (223 MB), `lanes/machine-ux` (181 MB), `lanes/shared-real-server-e2e` (223 MB).

Kept, with size, not ours to prune or still possibly in use (these are plain directory copies, not git worktrees, so branch is inferred from the name):

| Directory | Size | Note |
|---|---|---|
| `calternal-ai-lanes/v2-renderer-product` | 5.5 GB, all of it `target` | a process referencing it was running; ask before removing the target |
| `lanes/space-vm-wake` | 2.7 GB | protected lane |
| `lanes/v2-host-harness` | 1.3 GB | protected lane |
| `lanes/vm-measure` | 0.95 GB | lane kept |
| `lanes/plugin-memory` | 0.87 GB | protected lane |
| `lanes/turn-meta` | 0.78 GB (181 MB `node_modules`) | branch `lane/turn-meta` still exists unmerged locally |
| `lanes/search-private` | 0.7 GB (180 MB `node_modules`) | branch unknown |
| `lanes/deploy`, `lanes/e2e-dev`, `lanes/ui-notes-live`, `lanes/ui-notes-port`, `lanes/ui-shell`, `lanes/v2-host-items`, `lanes/v2-host-integration` | `node_modules` 156 to 267 MB each | branches kept or protected; prune after those lanes are archived |
| 30 small directories (evidence logs, plugin-api-v2 revisions, night-N copies, and so on) | under 400 MB each | no `target` or `node_modules`; nothing to prune |

No `target` directory exists in any of the dev-01 lanes except `v2-renderer-product`.

## Simplification opportunities

- Branches named `lane/*-wip`, `lane/*-keyed-*` and `stack/night-*` are experiments that never got an issue; a rule that experiments live on one `wip/` prefix would let them be listed and swept in one command.
- The lane worktrees in `Developer/Worktrees` are cut for every lane but never removed on merge; having the coordinator remove the worktree and local branch at merge time (as the AGENTS rule already says) would leave nothing for a housekeeping pass.
- `/tmp/coord-*` and `/tmp/calternal-v2-rev*` worktrees disappear on reboot and leave dangling registrations; use `Developer/Worktrees/` for them too.
- Many issues have no checklist at all (#69, #73, #93, #94, #96, #101), which makes "is it finished" impossible to tell mechanically.
