# Calternal Mail data plugin

Install this package with the reviewed package workflow in
[Author and install a TypeScript plugin](../../docs/plugin-authoring.md).
Enable it in your personal space. Its named `inbox` connection reaches a
public IMAPS mail server through the Calternal host and uses a password entered
in Calternal's protected prompt. The plugin has no visual surface.

| Operation | Input | Result |
| --- | --- | --- |
| `calternal-mail.add-account` | `host`, `port`, `username` | Local account item ID and revision, plus `ready` or a `secret_request_id` for the protected account prompt. Use the returned `id` as `account_id`. |
| `calternal-mail.list-accounts` | Optional `cursor` | Up to 16 local account summaries and a continuation. |
| `calternal-mail.list-folders` | `account_id` | Up to 200 folder names, delimiters and selectability markers, with a truncation flag. Keep names in their IMAP wire encoding when requesting a folder. |
| `calternal-mail.list-headers` | `account_id`, `folder`, optional `cursor` | Up to 20 headers, most recent first, the mailbox's `uid_validity` and a continuation. Use the cursor with the same account and folder. Refresh from page one to pick up mailbox changes. |
| `calternal-mail.read-message` | `account_id`, `folder`, `uid_validity`, `uid` | Header, plain text where available, attachment names/types/encoded sizes and a local snapshot item ID and revision. HTML appears as an optional sealed `frame_id`. |
| `calternal-mail.mark-read` | `account_id`, `folder`, `uid_validity`, `uid` | Sets `\\Seen` idempotently and confirms the new flag from the server. |
| `calternal-mail.open-snapshot` | `item_id` | Current snapshot reference and a fresh sealed frame ID when it has HTML. |

Use a request ID for add, read-message and mark-read. If the host recovers a
committed item write after interruption, it can replay the item ID and
revision alone. The protected prompt remains available through the usual
account inputs surface. Use `open-snapshot` to refresh an expired frame.
Resolve frames through `plugins.network.content.get` and display them in the
empty-sandbox iframe from the plugin guide.

## Storage and bounds

The selected message is saved as a human-readable RFC 5322/MIME `.eml`
snapshot under Mail Messages. Markdown cannot preserve email headers,
transfer encoding and separate text and HTML alternatives as native mail
clients expect. A snapshot materializes the fetched inline alternatives and
selected original headers. The mailbox holds the full original, including
attachments. Attachments are listed from IMAP BODYSTRUCTURE and their content
is never requested. Only explicitly opened messages are kept. A repeated
open reuses the snapshot indexed by account, folder, UIDVALIDITY and UID.
Snapshots and account settings follow ordinary personal-space content and
erasure policy. Mail has no background synchronization or automatic
expiration. Retained bytes grow with the number of messages explicitly
opened. Passwords follow the protected account secret lifecycle, including
the 15-minute pending prompt expiry.

Each operation uses one verified TLS connection, with up to eight concurrent
Mail connections across the process. The host caps encrypted input at 256 KiB,
uses an eight-second connect and I/O deadline after bounded DNS resolution,
and caps a read at 100 MIME parts and 12 levels. Each selected part is capped
at 16 KiB before and after decoding and each MIME header at 4 KiB. Oversized
messages are refused. Read-only operations use `EXAMINE` and `BODY.PEEK`.
Mail supports password-based IMAPS. OAuth-only accounts need a future
connection type. The worker has no SMTP, send or attachment download
operation.

The manifest requests anonymous remote content for the sealed renderer.
HTML sanitization and asset reference rewriting are active. The public
plugin host currently leaves those media references unloaded.

See [the implementation and authoring report](../../docs/mail-plugin.md).
