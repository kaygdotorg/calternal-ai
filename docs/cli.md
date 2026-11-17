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
| Lists | Aligned columns with an upper-case header line, two spaces between columns. An empty list prints one sentence saying so and what to do. |
| Single things | Labelled `label:  value` lines, values aligned. Values are never cut. |
| Width | `COLUMNS`, else the terminal's width, else unlimited (pipes get untruncated tables). Only when a table is wider than the terminal is its widest column shortened (to at least 12 characters, ending in `…`). |
| Colour | Bold table headers only, and only on a terminal with `NO_COLOR` unset and `TERM` not `dumb`. Never with `--json`. |
| Time | Human mode shows the local zone: `2026-11-17 07:12:03 +01:00`. JSON carries ISO 8601 (RFC 3339, UTC) exactly as the server or database gives it. |
| Errors, human | stderr: `error: <what happened>` then `hint: <next step>`. Nothing on stdout. |
| `--json` | stdout holds exactly one pretty-printed JSON document, for failures too; stderr stays empty. Success: `{"ok": true, "data": {…}}` (plus `"request_id"` for `cl` operations). Failure: `{"ok": false, "error": {"code", "message", "hint", "details"?}}` (plus `"request_id"` when known). `hint` is a string or `null`. Object keys are always printed in alphabetical order (not the order of `cli-shapes.json`), whatever the build; fields are only ever added, never renamed. |
| Usage errors | A command line that does not parse is an error with code `usage` and exit status 2, as JSON too when `--json` appears in the arguments. `--help` and `--version` print text and exit 0. |
| Secrets | Never printed, in either mode, except where the command exists to issue one: `calternal machines code` prints the new enrollment code, once. Tokens are accepted from environment variables or owner-only files, never from arguments (`cl`), and the machine key is never printed. |

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

Global option on every command: `--json`.

| Command | Purpose | Human output | `data` (JSON) |
|---|---|---|---|
| `serve --appdata DIR --userdata-ssd DIR --userdata-hdd DIR [--create] [--port N] [--web-dir DIR] [--development] [--origin URL] [--trusted-proxies CIDRS] [--plugin-root DIR] [--node PATH] [--machine-lease-ttl-ms N] [--machine-poll-hold-ms N]` | Run the server on 127.0.0.1 until stopped. The roots are validated and locked first: `--create` pairs empty or missing roots on first setup; missing, swapped or foreign roots are refused (exit 1) before anything is created. Prints one startup block, then logs to stderr. | Title line, then `url`, one `<name> token file` per development token (paths only, never tokens), `startup info`. | `profile`, `url`, `token_files` (name → path), `startup_file` |
| `machines code (--pool \| --account UUID) --data-dir DIR [--minutes N]` | Issue a one-time machine enrollment code (default 60 minutes). | `code`, `owner`, `expires` (local), then two sentences on using it. | `code`, `owner` (`pool` or `account:<uuid>`), `expires_at` |
| `machines list --data-dir DIR` | List enrolled machines, revoked ones included. | Table `ID OWNER NAME PLATFORM STATE` (state `enrolled`, `draining` or `revoked`), or "No machines are enrolled…". | `machines[]`: `id`, `owner`, `name`, `os`, `arch`, `state` |
| `machines revoke MACHINE --data-dir DIR` | Revoke a machine; a running server applies it within a second. | One sentence. Unknown machine: `not_found`. | `machine`, `revoked` |
| `machines pool-grant ACCOUNT [--remove] --data-dir DIR` | Allow (or stop allowing) an account to use pool machines. | One sentence. | `account`, `pool_access` |
| `plugin-digest [--plugin-root DIR]` | Hash each trusted worker bundle and say whether `trusted-plugins.json` pins it. | Table `PLUGIN WORKER SHA-256 STATE` (`pinned`, `NOT PINNED`, `unreadable: …`). | `workers[]`: `id`, `worker`, `sha256` (or null), `pinned`, `state` |
| `machine enroll --server URL --code CODE [--dir DIR] [--name LABEL]` | Exchange a one-time code for a machine identity; saves credentials (0600). `--code` also reads `CALTERNAL_ENROLL_CODE`. | `machine`, `owner`, `server`, `credentials` (path). | `machine_id`, `owner`, `server`, `credentials_file` |
| `machine status [--dir DIR]` | Show this machine's identity and server. | Same block as `enroll`. Not enrolled: `not_found`. | as `enroll` |
| `machine run [--dir DIR] [--backend podman\|process] [--image REF] [--podman PATH] [--max-sandboxes N]` | Connect to the server and run assigned threads until stopped (SIGINT/SIGTERM). Logs to stderr. | On clean stop: `Machine stopped.` Revoked: error `revoked`, exit 3. | `stopped` |

`machine enrol` no longer exists (renamed `machine enroll`, no alias); the HTTP path is
`/api/v1/machine/enroll`. Prefer `CALTERNAL_ENROLL_CODE` or `--code -` over `--code CODE`:
an argument is visible in the process list for the few seconds it runs.

Examples:

```console
$ calternal machines list --data-dir /srv/calternal
ID                                    OWNER                                         NAME       PLATFORM       STATE
aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa  pool                                          build-box  linux/x86_64   enrolled
bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb  account:cccccccc-cccc-4ccc-8ccc-cccccccccccc  laptop     macos/aarch64  enrolled

$ calternal machines code --pool --data-dir /srv/calternal
code:     ABCD-EFGH-JKMN-PQRS
owner:    pool
expires:  2026-11-17 08:12:03 +01:00

The code works once. Give it to the machine's owner now; it is not shown again.
On that machine: calternal machine enroll --server URL --code CODE

$ calternal machines revoke dddddddd-dddd-4ddd-8ddd-dddddddddddd --data-dir /srv/calternal
error: no machine dddddddd-dddd-4ddd-8ddd-dddddddddddd
hint: `calternal machines list --data-dir DIR` shows the machines and their ids.

$ calternal machines revoke dddddddd-dddd-4ddd-8ddd-dddddddddddd --data-dir /srv/calternal --json
{
  "error": {
    "code": "not_found",
    "hint": "`calternal machines list --data-dir DIR` shows the machines and their ids.",
    "message": "no machine dddddddd-dddd-4ddd-8ddd-dddddddddddd"
  },
  "ok": false
}
```

## `cl`

Global options: `--base-url`, `--token-file` (also `CALTERNAL_URL`,
`CALTERNAL_TOKEN_FILE`, `CALTERNAL_TOKEN`), `--request-id`, `--json`. Every
operation result carries the request identity; reuse `--request-id` only with the
identical command to retry safely.

| Command | Purpose | Human output | `data` (JSON) |
|---|---|---|---|
| `describe` | The server's protocol versions, your principal and spaces, and the operations it offers. | `protocol`, `genui`, `principal`; table `SPACE ROLE`; table `OPERATION ACCESS DESCRIPTION`. | the describe result: `protocol_version`, `genui_version`, `principal`, `operations[]` (with schemas) |
| `notes list [--space ID]` | List notes. | Table `ID TITLE UPDATED REVISION`, or "No notes in this space." | `notes[]` |
| `notes get ID` | Show one note. | `id`, `space`, `title`, `revision`, `updated`, blank line, the Markdown. | `note` |
| `notes create --title T (--markdown M \| --markdown-file F)` | Create a note. | As `notes get`. | `note` |
| `notes update ID --base-revision R [--title T] (--markdown M \| --markdown-file F)` | Replace a note's content against a known revision. | As `notes get`. A stale revision is `conflict`. | `note` |
| `notes history ID [--limit N] [--cursor C]` | Earlier versions of a note. | Table `REVISION TITLE UPDATED`, then "More versions: repeat with --cursor C" when there are more. | `versions[]`, `next_cursor`? |
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
error: revision is stale
hint: Fetch the current version (`cl notes get`), then retry with its revision.
```

## Tests

Snapshots (`tests/golden/` in each crate) cover human and JSON output of every
command and an error case each: `crates/cli/src/render.rs` (all of `cl`),
`crates/machine/src/cli.rs` (the machine role) and
`crates/server/tests/cli_golden.rs` (the real `calternal` binary, with the time zone
forced to UTC+1 and codes, ids, paths and ports replaced by placeholders).
`UPDATE_GOLDEN=1` rewrites them; review the diff.
