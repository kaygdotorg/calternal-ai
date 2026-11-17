# Async native client bridge

UniFFI exposes `NativeClient.describe`, `resolve` and `call` as async Swift APIs.
`NativeRequest` validates generated-contract JSON and allocates one stable request
ID before dispatch. Retain its envelope and identity after uncertain outcomes;
transport retries reuse the exact request (three attempts, 100/200 ms backoff).
No API, authentication, validation or protocol error is automatically retried.
`NativeCancellation` cancels HTTP/body/retry waiting locally; committed mutations
are not undone. A 401 surfaces `NativeError.ReauthenticationRequired`.

Platform code obtains session credentials through trusted passkey/OIDC endpoints
and stores them in its secret store. Supply the credential to `NativeClient`;
never log it. The preview transport accepts loopback HTTP only and also accepts
explicit disposable development tokens. This library does not implement passkey
UI, a Keychain adapter or automatic session refresh.

The synchronous schema/request/GenUI validation helpers remain available for
background contract tooling. All JSON helpers copy input/result bytes. Nothing
per keystroke, composition event, selection change, frame, animation or layout
crosses UniFFI. Native editors submit coalesced transactions off the UI thread;
stream rendering will require batched updates. Sync, offline cache and outbox
persistence remain unbuilt; see [the client design](../client/SYNC.md).

Build and generate on an authorized dev VM (use `.dylib` on macOS):

```sh
scripts/with-toolchain.sh cargo build -j4 -p calternal-bindings --features uniffi
target/debug/calternal-bindgen generate --library target/debug/libcalternal_bindings.so --language swift --out-dir crates/bindings/generated/swift
```

The Swift library/test target and disposable real-server harness are under
[CalternalCore](../../apps/apple/CalternalCore/README.md). Generated code must
match its linked library. A successful Linux generation alone is not Apple
execution or native rendering qualification.

`NativeClient.subscribe` restores an account-scoped map of space bookmarks and
returns `NativeLive`. Its async `next`/`accept` pair lets Apple apply state and
persist its bookmark before advancing the Rust reconnect cursor. The shared
`NativeCancellation` stops socket waits and reconnect backoff. `NativeLive.frames`
in the Swift package adapts this pull stream to `AsyncThrowingStream`; a slow
consumer closes with an explicit error and replays on reconnect rather than
silently dropping entries. There is no client merge engine or account draft API.
