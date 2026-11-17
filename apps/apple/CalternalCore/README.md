# CalternalCore bridge library

Library and real-server test target only; no UI views. `NativeClient` exposes
async describe, resolve and generic contract operations. `NativeRequest` allocates
one ID before dispatch; retain it and its envelope after cancellation/transport
failure. `NativeCancellation.cancel()` stops the local wait and retry backoff;
it cannot undo server effects. The handwritten Swift overloads connect Task cancellation to that token; UniFFI
0.29's generated async functions do not do this automatically. Explicit handles
allow cancellation across operation groups. A 401 throws
`NativeError.ReauthenticationRequired` for the trusted sign-in flow.

Credentials come from trusted passkey/OIDC endpoints and platform secret storage,
or disposable development token files in this harness. Never log them. The
preview transport currently permits loopback HTTP only. JSON values use the core
contract schema; generated Swift records are not an independent wire contract.

Build and generate on the authorized macOS VM, in its isolated client-core copy:

```sh
scripts/with-toolchain.sh cargo build -j4 -p calternal-server -p calternal-bindings --features calternal-bindings/uniffi
python3 crates/bindings/generate-swift.py
python3 apps/apple/CalternalCore/test-server.py
```

The generated Swift and C files must match the linked Rust library. SwiftPM uses
`target/debug` by default; set `CALTERNAL_RUST_LIB_DIR` for another build directory.
The test launches its own port-0 server and deletes its disposable data at exit.

Editor composition, selection, keystrokes, layout, animations and per-frame
rendering stay native. Submit coalesced edit transactions off the UI thread;
this JSON boundary copies payloads and is not a streaming editor. Journal sync,
offline cache, Keychain integration and native rendering are unbuilt.

On a Command Line Tools host without XCTest, run the same assertions with:

```sh
python3 apps/apple/CalternalCore/test-server.py --probe
```

`BridgeChecks` is shared by the XCTest adapter and `CalternalCoreProbe`; the latter
proves execution without claiming a successful `swift test`. The package uses the
inherited macOS 26.0 minimum. Other Apple platform qualification remains separate.

Space journals are available through `NativeClient.subscribe` and the cancellable
`NativeLive.next`/`accept` pair. `NativeLive.frames(cancellation:)` wraps this in
`AsyncThrowingStream` with one buffered event. Overflow stops explicitly; reconnect
uses the accepted bookmark. Apple owns persistence: apply/invalidate cache state,
persist its bookmark, then accept the event. Supply restored bookmarks when opening
the stream. No UI, Keychain storage or account draft/pending API is added here.
