# Person requests

The account's request inbox uses the ordinary operation envelope and the
existing [account feed](account-input.md). The server derives the recipient from
authentication and checks current space access before delivery. Other members
of a shared space see the turn's waiting state through its job stream. The
request itself and the submitted answer are private to the recipient.

| Operation | Input |
| --- | --- |
| `requests.list` | Empty object. Returns questions, approvals and secret prompts across the account's accessible spaces. |
| `requests.ask` | `form`, a GenUI v2 program containing one `Form`. The server derives the thread and space from the running thread credential. |
| `requests.answer` | Question `id` and `answer` as a JSON object of form values. |
| `requests.approve` | Approval `id` and `lifetime` of `once` or `this_thread`. Requires a fresh passkey proof on an installation session. |
| `requests.deny` | Approval `id`. |
| `space.instructions` | `space_id` and optional replacement `markdown`. Requires `space.settings` for both reading and writing. |

Questions use the existing GenUI catalog and are checked for embedded actions.
Approvals use a fixed trusted form. Each approval detail carries
`lifetimes` in both the list and account feed. The server uses the
same choices when validating a submitted decision. Secret prompts list metadata and an expiry
time. Values still enter through the trusted secret-provide endpoint. Thread
credentials can raise a question or request authority for their own turn.
They cannot list, answer, approve or deny a person's requests.

```sh
calternal requests list --json --token-file <installation-token-file>
calternal requests answer <id> --answer '{"release":"1.0"}'
calternal requests approve <id> --lifetime once
calternal requests deny <id>
```

Use `CALTERNAL_TOKEN_FILE` to supply the credential on subsequent commands.
Use the original `--request-id` and the same decision input to retry.

## Waiting and approval lifetimes

A turn that raises a question or approval stores a waiting checkpoint with the
request and its current machine. It releases the machine lease before completing
the transition, which fences old output and queues process shutdown while
preserving the workspace. The existing machine protocol's local lease TTL is
90 seconds if the machine loses contact. The turn consumes no execution slot
while waiting. On resolution, the same job is eligible to run again only on
the recorded machine. Ordinary placement can wait up to 600 seconds before
reporting interruption. A resolved form or decision is passed privately to the
provider as context for resumption.

Questions and approvals have no pending time limit. Secret prompts expire
after 15 minutes. There are at most 32 stored person-request and grant rows per
thread across its accounts, and 256 per account, including completed decision receipts.
An action or form and a form answer are independently limited to 16 KiB of
serialized JSON. Adding a request prunes completed answers, denials,
cancellations and used once claims after the receipt window
(`CALTERNAL_RECEIPT_RETENTION_DAYS`, seven days by default).

A once approval is spent in the protected admission transaction before the
action runs. This applies to reads and writers, including operations that
have no receipt of their own. An identical retry can return a saved response.
A failed, unconfirmed or expired result requires a new approval before another
execution. Receipt expiry never makes a spent approval usable again.

Thread approvals survive new turns for reads, reversible changes, and external
actions with identical canonical input. Irreversible and access-widening actions
only allow once approval and bind the request identity. A person's approval
rotates the parked turn's credential. Rotation at action consumption remains
a separate credential delivery decision.

Only the turn currently waiting on a request can accept a new decision.
Stopping, archiving, deleting or superseding a turn cancels its pending
requests. Archive or deletion also removes thread grants. Leaving a space,
removal, or losing space read access erases the former member's protected
requests, grants and saved thread authority in the membership commit.

## Standing instructions

`AI/AGENTS.md` has one writer. Both operations on the file require the Manager
or Owner capability `space.settings`. The file remains reserved from ordinary
content placement and writes. The replacement accepts up to 16 KiB of Markdown
and declares irreversible risk for replacement. It uses the per-space content
lock, durable outbox, descriptor-relative replacement and journal receipt. The journal event and receipt omit the text.
The previous filename is ignored.

Requests that supply a secret value or approve access require a fresh passkey
proof. A stale proof returns HTTP 403 with `passkey_reverification_required`.
Verify with a passkey and retry the action.

For `secrets.request`, choose a name from 1 to 96 ASCII bytes. Start with a
lowercase letter or digit, then use lowercase letters, digits, dots, underscores,
hyphens or colons. Use a name such as `service-token` and retry if the server
refuses the name.
