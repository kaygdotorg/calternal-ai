# Native validation facade (scaffolding)

This optional UniFFI crate exports three synchronous JSON functions:
`contract_schema_json()`, `validate_request_json(String)` and
`validate_genui_json(String)`. Validation returns `{"valid":true,...}` or
`{"valid":false,"error":{"code":...,"message":...}}`; it uses the Rust core
validators and generated contract. Unknown operation inputs still require the
server's registered schema; the generic facade only knows frozen operations.

```sh
scripts/with-toolchain.sh cargo build -p calternal-bindings --features uniffi
target/debug/calternal-bindgen generate --library target/debug/libcalternal_bindings.so --language swift --out-dir crates/bindings/generated/swift
```

`generated/swift` contains a usable generated Swift facade, C header and module
map. Generation and Rust compilation were exercised on Linux; this is not an
Apple build/run or native rendering proof. No UI code is present. `swift-format`
was unavailable during generation, so generated Swift is unformatted.

Core has no network, async runtime or UniFFI dependency. `crates/client` owns
network effects; this facade has no client/session/outbox/credential adapter or
network operation to cancel. Cancelling a future Swift caller would not cancel
an independently running HTTP request or reverse a committed write. A real
native client must retain request identities and use the same protocol semantics;
platform credentials, explicit operation cancellation, offline/reconnect and
native renderer conformance remain later gates.
