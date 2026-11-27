# Runs and Claude Code harness findings - 2026-11-27

Lane D, slice 6. References: chat-first build plan slice 6; primitives-only host sections 6-7; format revision 9 sections 2.9, 2.20, 7.1, 7.4 and 8.1; lane C frame contract at origin/lane/plugin-process commit 499b5b3f. Reuse: crates/machine PodmanBackend, start_harness, provider_home, Sandbox exec/release, and plugins/calternal-claude-code adapter and Containerfile.

## Decisions and gaps

| Section | Finding and choice |
| --- | --- |
| Format 7.1; lane C run frame | The lane C example uses program ID calternal-chat-claude-code, but the revision 9 manifest declares calternal-provider-claude-code. Use the manifest ID. The executable wrapper in this plugin's programs folder binds it to the existing adapter in the image. |
| Format 8.1; lane C run handoff | The host records path is now on dev, and lane C is wiring s.call and s.run to it. Until that lands, follow the coordinator: keep the reply sink stub. This smoke test uses a generated run ID and ends at an in-memory ReplySinkStub. Run lookup, actor/space binding, session target resolution, send authority (acting person only), and stop authority (anyone who can read a target) are not proven by this machine-layer test. |
| Format 2.20 and 8.1 | The host resolves scope=session at start and fixes it to the current chat session. The test target is synthetic; the host must supply and enforce the real session target. |
| Format 7.4; coordinator set 11 | Send-and-start is one Chat call under one request key. Chat send must include the chosen harness, append the message, then call its declared start operation with the append receipt's host-issued canonical link. Claude StartInput must accept that link and use it as the run's first input. The fixture operations do not do this; record the follow-up without editing fixtures. |
| Format 2.9; coordinator set 11 | Unread state is an anchor, not a timestamp: count ledger records after state.read through in ledger order. The chats kind still types read through as time and compares at >; both fixture lines need updating by their owner. |
| Coordinator set 11; rev 9 slot rules | Composer and harness slots take inputs from one typed operation-input declaration, fixed per format version with no optional extras or negotiation. The fixtures need this wiring; do not edit them. |
| Coordinator set 12; lane C frames | Trusted slot selection admits one declared operation. Receipts carry a canonical link; change selectors are discovered once at install; oversized frames are refused; installed code stays off outside tests until OS sandboxing; workers recycle when the actor changes. This smoke test does not enable operations.ts or implement host/worker policy. |
| Primitives-only host section 6 | No verified mediated egress for api.anthropic.com exists. Podman harnesses use --network=none, including nested Podman; the fake provider needs no network. Ask the coordinator which mediated route is approved before enabling a real provider. No real Claude command or sign-in was used. |
| Coordinator set 13 | Append tagging, developer-only local installs, and retention do not change this machine slice; durability and receipts remain with the host records lane. |

## Fixture lines for their owner

- plugins-v2/calternal-chat/operations.ts: include the selected harness; use the new record's canonical link to start it under the same request key.
- plugins-v2/calternal-chat-claude-code/operations.ts: accept the prompting record link and send it as the run's first actor input.
- plugins-v2/calternal-chat/kinds/chats.md: make read through an anchor and count after it in ledger order.
- Slot declarations: reference the single typed operation-input declaration.

## Verification and hot-path counts

The focused gate passed on claude@calternal-dev-01-compute02 in ~/Developer/lanes/runs-harness under the shared lock and nix develop:
- npm ci and npm run build:plugins passed.
- The fake echo image built with test/build-echo-image.sh.
- cargo test -p calternal-machine --test runs_harness -- --nocapture passed 1 test after merging host records from dev.
- cargo fmt --check -p calternal-machine passed.

For the path exercised, host ledger reads: 0; SQLite reads: 0; durable record flushes: 0; in-memory reply-sink appends: 1. These counts are structural because the host record route is not called; they are not instrumented performance measurements. Provider-profile and container lifecycle I/O are not counted. Lane B measured two flushes for a host append; this test makes no append durability claim. Podman engine removal is bounded at 5 seconds after one observed teardown took 2.92 seconds; the adapter Exit must still arrive within 2 seconds after release returns.
## Simplification opportunities

- If this echo fixture grows, merge its echo and process-hold modes with the existing adapter fake instead of maintaining two provider simulators.
- Once the host installs a complete plugin bundle as the program image, the test image builder and wrapper may be unnecessary; keep the manifest ID binding.
- Keep actor, space, target and run-ID checks in the trusted host service, not duplicated in the generic sandbox layer.
