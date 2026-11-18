# Browser push

Calternal sends notifications for the initiating person's thread when a
committed request needs them or a turn finishes. Subscribing is opt-in.
The browser receives the current title and a status. Message text, request
details, tool output, thread IDs, credentials, and account IDs stay out of
the payload. The browser can open the signed-in app from a notification.

## Operations

Call these through `POST /api/v1/operations` using a current browser session.
The existing installation UUID identifies the device. No operation accepts
an account ID.

| Operation | Input | Result data |
| --- | --- | --- |
| `push.devices` | `{}` | `{application_server_key, devices}` |
| `push.subscribe` | `{endpoint, p256dh, auth}` | `{application_server_key, devices}` |
| `push.unsubscribe` | `{device_id}` | `{removed: boolean}` |
| `push.test` | `{}` | `{queued: true}` |

`application_server_key` is the base64url public VAPID key. Pass its decoded
bytes to `PushManager.subscribe` with `userVisibleOnly: true` in a user gesture.
Send the returned endpoint and browser keys with `push.subscribe`. The keys
use unpadded base64url. Keep the browser's registration and the server
registration in sync when disabling notifications or signing out.

Each device row contains `device_id`, `name`, `current`, and
`expires_at_ms` in Unix milliseconds. Listing omits revoked or expired
installations and never returns endpoints or browser keys. Unsubscribing a
missing or foreign device returns `removed: false`. The test targets the
current registered installation and reports admission to the queue.

The decrypted service-worker payload is `{title, status}`. Render
`needs_you` as “needs you”, `finished` as “finished”, and `test` as a test
notification. The test uses the title “Calternal”. The wire shape is also
exported as `PushPayload` in the TypeScript contract.

## Storage and delivery

Each account keeps up to eight subscriptions. Re-registering the same
installation renews its 90-day expiry. Expired entries stop delivering
immediately and are swept at startup and daily. Account erasure removes the
account-owned store. A test has a one-minute cooldown per installation.

`accounts/<account UUID>/web-push.db` starts at SQLite version 1, using the
shared upgrade runner. The server generates its private VAPID key at first
HTTPS startup and stores it as `system/web-push-vapid-v1.der` in appdata with
mode 0600. Preserve the key in backups. Replacing it requires clients to
resubscribe. Only the public key is returned, through the signed-in UI API.

A bounded in-memory queue and four background delivery workers drop stale
notices after a minute. A full queue can drop a thread notification. The
queue is best effort across restarts. The worker rechecks the account's
access, the installation and the stored subscription. HTTPS delivery uses
the shared public-address guard, an explicit browser-provider hostname
list, no redirects, an eight-second timeout and a five-minute relay TTL.
A relay 404 or 410 removes the attempted subscription generation only.

The request path performs no relay calls. A device list reads the protected
installation inventory and one bounded per-account subscription query.
Registration and test admission use short account-local SQLite writes.
