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
Approvals use a fixed trusted form. Secret prompts list metadata and an expiry
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
serialized JSON. Completed answers, denials and used once claims are pruned
when adding a request after the receipt window
(`CALTERNAL_RECEIPT_RETENTION_DAYS`, seven days by default).

Once approvals claim a matching operation's request ID and input fingerprint in
the protected authority transaction. A concurrent attempt with a different
identity cannot use the same grant. A retry of the claimed identity follows the
operation's receipt. Receipt-capable native actions and reads are wired through
this path. A read's first returned result is recorded in the ordinary journal
receipt before delivery. The current preview leaves once approvals for
unreceipted writers unsupported.

Thread approvals survive new turns and are erased with archive or deletion.
Sensitive action scopes bind operation and input. Irreversible operations and
access widening also bind the original request identity. The protected commit
for a person's approval rotates the thread credential. The current claim
implementation fences reuse of a once approval in the authority table, with
credential rotation at the approval decision. Rotation at action consumption
requires the remaining credential delivery work described in the lane report.

## Standing instructions

`AI/AGENTS.md` has one writer. Both operations on the file require the Manager
or Owner capability `space.settings`. The file remains reserved from ordinary
content placement and writes. The replacement accepts up to 16 KiB of Markdown
and declares irreversible risk for replacement. It uses the per-space content lock, durable outbox, descriptor-relative file
replacement and journal receipt. The journal event and receipt omit the text.
The previous filename is ignored.
