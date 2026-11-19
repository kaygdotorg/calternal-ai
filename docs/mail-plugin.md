# Mail plugin data layer

Calternal Mail uses a TypeScript worker and the reviewed package workflow.
No visual component is part of this package. A person's IMAPS endpoint and
message snapshots live in their personal space. The password is requested
through the protected account input surface and bound to the package,
connection name, normalized host, port and username. The host resolves
through the wallpaper network address policy, connects to a checked IP,
verifies the certificate against the configured TLS hostname, and then
releases the secret to the host-owned login consumer. The worker receives no
password or handle. The broker tags secret access with the plugin actor.
The Mail manifest and worker reuse the ordinary package, activation,
capability, content journal and sealed frame APIs. No application database or
existing file format changes. The new kinds use ordinary content provisioning.

The [package README](../plugins/calternal-mail/README.md) describes inputs,
results, operation bounds, and snapshot retention. A message's stable remote
identity is its account, folder, UIDVALIDITY and UID. The header feed walks
message sequence windows, which are a live mailbox view. Moving or expunging
messages during paging can repeat or omit headers until the caller starts
again from the first page. Reads and writes use the stable remote identity.

## Plugin-authoring friction

| Public path | What the Mail author had to do |
| --- | --- |
| Named connections | The guide described the design but the strict manifest and worker protocol rejected it. The host and SDK now accept one named `mail_connection` and the host-owned `mail_v1` IMAPS protocol. Review and activation must cover the scope. |
| Secret handles | Static manifest names were space-scoped and returned a token to the worker. User-supplied endpoints need a private account prompt and an exact-destination consumer. The broker now holds that binding and initiates the existing secure prompt itself. |
| Preview and local network | Package fixture replies exercise the worker but cannot prove TLS or IMAP behavior. Private addresses are unavailable under the public grant. The host integration fixture pins a generated certificate inside tests and uses a local-only listener. Production keeps the normal trust store and address guard. |
| Binary kind and sealed frames | The prepared-content API originally accepted only text kinds, and the TypeScript package did not export its documented helper. It now exports `prepareContent` and accepts `mail_html` from a declared `.eml` snapshot. The item broker can transport this UTF-8 MIME file but has no general binary streaming interface. |
| Remote images | Sealed frames rewrite asset references. The anonymous media-delivery endpoint is pending, so a Mail author can declare the scope but cannot display remote images in this stack. |
| Item index and retention | The worker has exact labels and ordinary content writes. It lacks an atomic upsert keyed by the source identity, a quota primitive and a retention hook. Concurrent first opens with distinct request IDs can create duplicate snapshots. |
| Response schemas and retries | A crash between the committed item write and the operation result can replay a generic item ID and revision only. Consequently the operation result permits that form. The frame can be regenerated from the saved item. |
| Risk and time budgets | Manifest read operations must use `risk: read` even when contacting a provider. The connection declaration exposes the network scope during review. The usual three-second execution budget is too short for several serial IMAPS round trips. The host gives named Mail operations 15 seconds, with the stricter socket and byte limits enforced independently. |
| Error reporting | A refused callback cannot describe TLS, credential or mailbox failure. Redaction prevents provider text reaching the worker. An eventual typed host error could be more actionable while preserving that redaction. |
