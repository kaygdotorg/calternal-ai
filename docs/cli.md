# Command-line reference

Two programs, one rule: **every command prints structured, human-readable output by
default and accepts `--json`** for a stable machine-readable form carrying the same
information. All printing goes through one module, `calternal_cli::output`
([`crates/cli/src/output/`](../crates/cli/src/output/mod.rs)); a command builds a
reply or an error and never prints results itself.

- **`calternal`**: the single product binary (server, operator and machine roles).
- **`cl`**: the client for a running server (the development-preview CLI).

## Output conventions

| Topic | Rule |
|---|---|
| Lists | At a wide terminal, aligned columns with an upper-case header and two spaces between columns. At narrow widths, IDs shorten in the middle and text shortens at the end; the lowest-priority columns are then hidden with a count and a `--json` or wider-terminal suggestion. Below 48 columns, each row becomes labelled lines. Empty results say so and suggest a next step. |
| Single things | Labelled `label:  value` lines, values aligned by terminal-cell width. Values are never cut. |
| Width | On a terminal, `COLUMNS` takes priority over the terminal width. Below 48 columns, rows use labelled blocks. Piped and redirected output is never truncated, even if `COLUMNS` is set. Wide Unicode characters count as two terminal cells; control characters in cells become spaces. |
| Colour | Bold table headers and a dim next-step hint only on a terminal when `NO_COLOR` is unset and `TERM` is not `dumb`. Never with `--json`; no-colour terminals show hints as plain text. |
| Hints | A successful command may show one short `Next:` line on a terminal in human mode. `--no-hints` or `CALTERNAL_NO_HINTS=1` hides it. Piped output and `--json` never include success hints. |
| Help | Every command's `--help` includes a one-line purpose, examples, and `See also` commands. |
| Time | Human mode shows the local zone: `2026-11-17 07:12:03 +01:00`. JSON carries ISO 8601 (RFC 3339, UTC) exactly as the server or database gives it. |
| Errors, human | stderr: `error: <what happened and why, when known>` then `hint: <next step>`. Nothing on stdout. Wording may be friendlier than the unchanged JSON message. |
| `--json` | stdout holds exactly one pretty-printed JSON document, for failures too; stderr stays empty. Success: `{"ok": true, "data": {…}}` (plus `"request_id"` for `cl` operations). Failure: `{"ok": false, "error": {"code", "message", "hint", "details"?}}` (plus `"request_id"` when known). `hint` is a string or `null`. Object keys are always printed in alphabetical order (not the order of `cli-shapes.json`), whatever the build; fields are only ever added, never renamed. |
| Usage errors | A command line that does not parse is an error with code `usage` and exit status 2, as JSON too when `--json` appears in the arguments. `--help` and `--version` print text and exit 0. |
| Secrets | Never printed, in either mode, except where the command exists to issue one: `calternal machines code` prints the new enrollment code once. `calternal invites list` prints usable invite links for people authorized to copy them. Tokens are accepted from environment variables or owner-only files, never from arguments (`cl`), and the machine key is never printed. |

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Success. |
| 1 | The command ran and failed (the `error.code` says why). |
| 2 | Usage error: the command line does not parse. Nothing else uses 2. |
| 3 | `calternal machine run` only: the server revoked this machine. The systemd unit from `scripts/install-machine.sh` does not restart on 3. |

### Error codes (`error.code`)

Branch on these, never on `message`. `usage`, `invalid_input`, `unauthorized`,
`forbidden`, `not_found`, `conflict`, `too_large`, `unavailable`, `internal` (the
`cl` set, equal to the server's `ErrorCode`), plus `io` (a local file failed),
`failed` (the operation failed; the message says why) and `revoked` (machine role).

### JSON shapes

The exact shape of every command's `data` is machine-readable in
[`cli-shapes.json`](cli-shapes.json): leaves are `string`, `string?` (string or null),
`integer`, `bool`, `any`; an object has exactly the listed keys (a key ending in `?`
is optional); an array lists the element shape. The golden tests check every
`--json` document against it, so the file and the code cannot drift.

## `calternal`

Global options on every command: `--json`, `--no-hints`.

| Command | Purpose | Human output | `data` (JSON) |
|---|---|---|---|
| `serve --appdata DIR --userdata-ssd DIR --userdata-hdd DIR --installation-id UUID [--create] [--receipt-retention-days N] [--invite-retention-days DAYS] [--port N] [--web-dir DIR] [--development] [--origin URL] [--trusted-proxies CIDRS] [--plugin-root DIR] [--node PATH] [--machine-lease-ttl-ms N] [--machine-poll-hold-ms N]` | Run the server on 127.0.0.1 until stopped. The installation ID (`--installation-id`, or `CALTERNAL_INSTALLATION_ID` in the server's configuration) is required; the roots are validated and locked against it first: `--create` pairs empty or missing roots on first setup; missing, swapped or foreign roots are refused (exit 1) before anything is created. Prints one startup block, then logs to stderr. | Title line, then `url`, one `<name> token file` per development token (paths only, never tokens), `startup info`. | `profile`, `url`, `token_files` (name → path), `startup_file` |
| `machines code (--pool \| --account UUID) --appdata DIR --userdata-ssd DIR --userdata-hdd DIR [--minutes N]` | Issue a one-time machine enrollment code (default 60 minutes). | `code`, `owner`, `expires` (local), then two sentences on using it. | `code`, `owner` (`pool` or `account:<uuid>`), `expires_at` |
| `machines list --appdata DIR --userdata-ssd DIR --userdata-hdd DIR` | List enrolled machines, revoked ones included. | Table `ID OWNER NAME PLATFORM STATE` (state `enrolled`, `draining` or `revoked`), or a friendly empty result with the command to add one. | `machines[]`: `id`, `owner`, `name`, `os`, `arch`, `state` |
| `machines revoke MACHINE --appdata DIR --userdata-ssd DIR --userdata-hdd DIR` | Revoke a machine; it can no longer authenticate, and its leases end at once. | One sentence. Unknown machine: `not_found`. | `machine`, `revoked` |
| `machines pool-grant ACCOUNT [--remove] --appdata DIR --userdata-ssd DIR --userdata-hdd DIR` | Allow (or stop allowing) an account to use pool machines. | One sentence. | `account`, `pool_access` |
| `agent readiness --space-id UUID [--thread-id UUID]` | Read cached placement capacity and provider sign-in for the caller. | Adaptive machine and provider table, with the official sign-in command. | `machines[]`, `pool[]`, `check_ttl_ms` |
| `plugin-digest [--plugin-root DIR]` | Hash each trusted worker bundle and say whether `trusted-plugins.json` pins it. | Table `PLUGIN WORKER SHA-256 STATE` (`pinned`, `NOT PINNED`, `unreadable: …`), or a friendly empty result. | `workers[]`: `id`, `worker`, `sha256` (or null), `pinned`, `state` |
| `machine enroll --server URL --code CODE [--dir DIR] [--name LABEL]` | Exchange a one-time code for a machine identity; saves credentials (0600). `--code` also reads `CALTERNAL_ENROLL_CODE`. | `machine`, `owner`, `server`, `credentials` (path). | `machine_id`, `owner`, `server`, `credentials_file` |
| `machine status [--dir DIR]` | Show this machine's identity and server. | Same block as `enroll`. Not enrolled: `not_found`. | as `enroll` |
| `machine verify MACHINE_UUID --space-id SPACE_UUID [--provider ID] [--token-file FILE]` | Run one provider diagnostic on your machine or a pool machine you administer. | Sandbox status, provider installation and sign-in, and the provider's sign-in command. | `sandbox_started`, `provider` |
| `machine run [--dir DIR] [--backend podman\|process] [--image REF] [--podman PATH] [--max-sandboxes N]` | Connect to the server and run assigned threads until stopped (SIGINT/SIGTERM). Logs to stderr. | On clean stop: `Machine stopped.` Revoked: error `revoked`, exit 3. | `stopped` |

People, groups, and invitations use the HTTP session in an owner-only file
supplied with `--token-file` or `CALTERNAL_TOKEN_FILE`. Use `--base-url` to choose
the server and keep the same `--request-id` when retrying an uncertain write.
These commands preserve Unix milliseconds in JSON wherever the HTTP contract
uses them. Human invite output shows expiry in the local time zone.

| Arguments | Purpose | Human output | JSON data |
|---|---|---|---|
| `people members --space SPACE [--cursor CURSOR]` | List direct people and group grants in a space. | ID, name, role, recipient type, group member count. | `members`, `groups` |
| `people add --space SPACE (--account ID or --group ID) --role ROLE` | Grant a direct role to a person or group. | Grant confirmation. | empty object |
| `people set-role --space SPACE (--account ID or --group ID) --role ROLE` | Change a direct grant's role. | Save confirmation. | empty object |
| `people remove --space SPACE (--account ID or --group ID)` | Remove a direct grant. | Removal confirmation. | empty object |
| `people transfer --space SPACE ACCOUNT` | Transfer space ownership to a current member. | Transfer confirmation. | empty object |
| `people roles` | Show role definitions for a space. | Role labels and capability tables. | `roles` |
| `people set-profile NAME` | Update your own display name. | Account ID and name. | `account_id`, `name` |
| `people list [--cursor CURSOR]` | List server accounts with the people administration capability. | ID, name, role, state. | `people` |
| `people disable ACCOUNT` | Disable the account and sign out its sessions. | Account ID and disable confirmation. | `account_id`, `disabled` |
| `groups list [--group GROUP] [--cursor CURSOR]` | List server groups and current member IDs. | Group and member tables. | `groups` |
| `groups create NAME` | Create a local group. | Group ID. | `group_id` |
| `groups rename GROUP NAME` | Rename a local group. | Group ID. | `group_id` |
| `groups delete GROUP` | Delete a local group and its grants. | Group ID. | `group_id` |
| `groups add-member GROUP ACCOUNT` | Add a person to a local group. | Group ID. | `group_id` |
| `groups remove-member GROUP ACCOUNT` | Remove a person from a local group. | Group ID. | `group_id` |
| `invites create --space SPACE --role ROLE [--expires-at MILLISECONDS]` | Issue one invite. The default expiry is seven days. | Invite ID and expiry. | `invite_id`, `expires_at` |
| `invites list --space SPACE [--cursor CURSOR]` | List invitations and copy usable links. | Invite state and complete usable links. | `invites` |
| `invites revoke --space SPACE INVITE` | Revoke an unused link. | Invite ID and revocation confirmation. | `invite_id` |
| `invites redeem --secret-file FILE` | Redeem a link as the signed-in person. Use `-` to read standard input. | Space ID. | `space_id` |

Directory pages display a continuation when more rows are available. Use it
with `--cursor`. For a group's member continuation, also pass `--group GROUP`.
The invite retention setting is `CALTERNAL_INVITE_RETENTION_DAYS`, seven days by
default from expiry, redemption, or revocation.

| Command | Purpose | Human output | Data |
|---|---|---|---|
| `notes list --space UUID [--tag TAG] [--cursor CURSOR]` | List up to 100 active notes, optionally under a tag. | Note table | `notes[]` |
| `notes lookup --space UUID PREFIX [--cursor CURSOR]` | Find live notes by title prefix. | Note table with link target | `notes[]` with `link_target` |
| `notes rename ID --base-revision REVISION --title TITLE` | Update heading, filename and links. | Note fields and Markdown | `note` |
| `notes delete ID` | Move a note to Trash. | Note fields and state | `note` |
| `notes restore ID` | Restore a note. | Note fields and state | `note` |
| `tags list --space UUID [--cursor CURSOR]` | List tags and note counts. | Tag and count table | `tags[]` |
| `tags lookup --space UUID PREFIX [--cursor CURSOR]` | Find tags by name prefix. | Tag and count table | `tags[]` |
| `trash list --space UUID [--cursor CURSOR]` | List trashed notes. | Note table with deletion time | `notes[]` |
| `trash purge ID` | Permanently delete a note in Trash. | ID and deletion state | `id`, `state` |

Each Notes, Tags and Trash listing or lookup command returns up to 100 rows.
Continue with the same arguments and `--cursor`. JSON output includes
`next_cursor` when another page is available. Pass `--cursor -` to read a
cursor from standard input, including values too long for a shell argument.

These commands accept `--base-url`, `--token-file`, `--request-id` and
`--json`. See [Notes operations](notes-operations.md) for bounds, permission
checks and Trash retention.

| Command | Purpose | Human output | `data` (JSON) |
|---|---|---|---|
| `requests list` | List private questions, approvals and secret prompts. | Adaptive table with kind, request ID, space, detail and allowed approval lifetimes. | `requests[]` |
| `requests answer ID --answer JSON` | Submit a question's form values. | Request ID and status. | `id`, `status` |
| `requests approve ID --lifetime once\|this-thread` | Approve using a fresh passkey session. | Request ID and status. | `id`, `status` |
| `requests deny ID` | Decline a pending approval. | Request ID and status. | `id`, `status` |

Both `calternal requests` and `cl requests` use the same operations. See the
[request contract](requests.md) for session and retry requirements.

The `machines` commands name the installation exactly as `serve` does: the three
roots and the installation ID (`--installation-id` or `CALTERNAL_INSTALLATION_ID`).
Machine authority (codes, enrollment, revocation, pool grants) is recorded in the
recovery log before it is acknowledged, and that log has one writer, so a command
never writes behind a running server: it asks that server over its owner-only admin
socket (`appdata/system/admin.sock`, or a private folder in the temp directory when
that path is too long), which applies the change at once. With no server running,
the command opens the installation itself through the storage guard (nothing is
created; missing or unpaired roots are refused) and attaches the recovery log first.

`machine enrol` no longer exists (renamed `machine enroll`, no alias); the HTTP path is
`/api/v1/machine/enroll`. Prefer `CALTERNAL_ENROLL_CODE` or `--code -` over `--code CODE`:
an argument is visible in the process list for the few seconds it runs.

Examples:

```console
$ calternal machines list --appdata /srv/calternal/appdata --userdata-ssd /srv/calternal/ssd --userdata-hdd /mnt/hdd/calternal
ID                                    OWNER                                         NAME       PLATFORM       STATE
aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa  pool                                          build-box  linux/x86_64   enrolled
bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb  account:cccccccc-cccc-4ccc-8ccc-cccccccccccc  laptop     macos/aarch64  enrolled

$ calternal machines code --pool --appdata /srv/calternal/appdata --userdata-ssd /srv/calternal/ssd --userdata-hdd /mnt/hdd/calternal
code:     ABCD-EFGH-JKMN-PQRS
owner:    pool
expires:  2026-11-17 08:12:03 +01:00

This code works once. Share it with the machine's owner now.
Next: run `calternal machine enroll` on that computer.

$ calternal machines revoke dddddddd-dddd-4ddd-8ddd-dddddddddddd --appdata /srv/calternal/appdata --userdata-ssd /srv/calternal/ssd --userdata-hdd /mnt/hdd/calternal
error: No machine with ID dddddddd-dddd-4ddd-8ddd-dddddddddddd was found.
hint: `calternal machines list` shows the machines and their ids.

$ calternal machines revoke dddddddd-dddd-4ddd-8ddd-dddddddddddd --appdata /srv/calternal/appdata --userdata-ssd /srv/calternal/ssd --userdata-hdd /mnt/hdd/calternal --json
{
  "error": {
    "code": "not_found",
    "hint": "`calternal machines list` shows the machines and their ids.",
    "message": "no machine dddddddd-dddd-4ddd-8ddd-dddddddddddd"
  },
  "ok": false
}
```

## `cl`

Global options: `--base-url`, `--token-file` (also `CALTERNAL_URL`,
`CALTERNAL_TOKEN_FILE`, `CALTERNAL_TOKEN`), `--request-id`, `--json`,
`--no-hints` (also `CALTERNAL_NO_HINTS=1`). Every
operation result carries the request identity; reuse `--request-id` only with the
identical command to retry safely.

| Command | Purpose | Human output | `data` (JSON) |
|---|---|---|---|
| `describe` | The server's protocol versions, your principal and spaces, and the operations it offers. | `protocol`, `genui`, `principal`; table `SPACE ROLE`; table `OPERATION ACCESS DESCRIPTION`. | the describe result: `protocol_version`, `genui_version`, `principal`, `operations[]` (with schemas) |
| `notes list [--space ID] [--cursor CURSOR]` | List notes. | Table `ID TITLE UPDATED REVISION`, or a friendly empty result with a command to create one. | `notes[]` |
| `notes get ID` | Show one note. | `id`, `space`, `title`, `revision`, `updated`, blank line, the Markdown. | `note` |
| `notes create --title T (--markdown M \| --markdown-file F)` | Create a note. | As `notes get`. | `note` |
| `notes update ID --base-revision R [--title T] (--markdown M \| --markdown-file F)` | Replace a note's content against a known revision. | As `notes get`. A stale revision is `conflict`. | `note` |
| `notes history ID [--limit N] [--cursor C]` | Earlier versions of a note. | Table `REVISION TITLE UPDATED`, then "More versions: repeat with --cursor C" when there are more; an empty result suggests opening the current version. | `versions[]`, `next_cursor`? |
| `plugins list` | Installed plugins for a space. | Table `ID NAME VERSION ENABLED GENERATION`. | `plugins[]` |
| `plugins enable PLUGIN` / `plugins disable PLUGIN` | Turn a plugin on or off in a space. | `id`, `name`, `version`, `enabled`, `generation`. | `plugin` |
| `plugins preview PLUGIN` | Show the plugin's GenUI view without changing anything. | One sentence, then the view as indented JSON. | `view`, `preview` |
| `invoke OPERATION --input JSON` | Call any operation by id. | The operation's data as indented JSON (the same text as `data` in `--json`). | the operation's own result (`any`) |

`note` and `plugin` are accepted spellings of `notes` and `plugins`. A transport
failure after the request was sent has `error.details` `{"outcome": "unknown", "retry": …}`
and a hint with the exact `--request-id` to reuse.

```console
$ cl notes list
ID                                    TITLE      UPDATED                     REVISION
11111111-1111-4111-8111-111111111111  Groceries  2026-11-17 07:12:03 +01:00  r-7

$ cl notes update 1111… --base-revision r-1 --markdown "x"
error: The saved version has changed: revision is stale
hint: Run `cl notes get ID` for the latest revision, then try again.
```

## Tests

Snapshots (`tests/golden/` in each crate) cover human and JSON output of every
command and an error case each: `crates/cli/src/render.rs` (all of `cl`),
`crates/machine/src/cli.rs` (the machine role) and
`crates/server/tests/cli_golden.rs` (the real `calternal` binary, with the time zone
forced to UTC+1 and codes, ids, paths and ports replaced by placeholders).
`UPDATE_GOLDEN=1` rewrites them; review the diff.

## Manual agents

`calternal agent check|run|stop|resume|discard` supervises an official CLI on an enrolled
machine. `agent mode` and `agent approve` require a freshly verified person
session. `run` and `resume` stream text/tool steps in human mode and NDJSON with
`--json`. The same binary exposes `describe` and `invoke` inside a sandbox using
its thread credential; no provider credential is passed to these commands.
See [manual turns](agent-turns.md) for flags, settings and isolation limits.

`calternal watch <space-uuid> --token-file <owner-only-file>` follows the server's
space journal over one SSE connection. `cl watch` accepts the same arguments.
With `--json`, each event is one compact `{ "ok": true, "data": ... }` line.
Human output uses labelled fields. Reconnect uses accepted signed bookmarks;
`--bookmarks <directory>` retains them in atomic `bookmark: value` Markdown files.
That directory must be exclusive to this server and signed-in account; discard it
on sign-out or account changes. Without the option, bookmarks stay in memory.
A reset stops watch: reload the authorized space before discarding its old bookmark.

`calternal agent discard <job-id>` keeps the account login home and reports pending
until the machines confirm deletion. Stop the latest turn first. Inspect a failed
cleanup before `discard <job-id> --retry`. A machine owner can stop its role and
use `calternal machine discard --account-id <uuid> --thread-id <uuid>` for retained
local work; `--json` returns the same confirmation and identifiers.
`calternal drafts` and `calternal pending` manage private account input through
the same shared operation route and output renderer as `cl drafts` / `cl pending`.
See [account input](account-input.md) for queue files, stable nested request IDs,
seven-day confirmation, examples and operator safety ceilings.

`calternal threads create|list|get|send|stop|rename|archive|unarchive|delete` operates on saved conversations using manifest-backed operations. See [threads](threads.md) for arguments, file format, retry behavior and current limits. Every subcommand supports `--json` and shared terminal hints.
