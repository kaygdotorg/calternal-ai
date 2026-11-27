# Runs and Claude Code harness findings - 2026-11-27

Lane D, slice 6. References: chat-first build plan slice 6; primitives-only host sections 6-7; format revision 9 sections 2.9, 2.20, 7.1, 7.4 and 8.1; lane C frame contract at commit 499b5b3f. Reuse: `PodmanBackend`, its harness profile and streamed exec, the existing Claude Code adapter and Containerfile, the host records route, and the slot-selected `s.run` dispatch.

## Status and decisions

The host CONNECT egress proxy and machine run service pass their dev VM gates with the fake provider. The host `s.run`/Chat append integration is the remaining step in this lane and is not claimed as verified here. No real Claude executable or provider sign-in was used.

| Section | Finding and choice |
| --- | --- |
| Format 7.1 and lane C run frames | The installed manifest binds `calternal-provider-claude-code` to the executable in this plugin's `programs/` folder. The wrapper starts the bridge and control channel, then delegates stream handling to the existing adapter in the image. |
| Format 8.1 | The machine library starts a fixed session target set, accepts input only from the starting actor, streams ordered output, and asks the host authorizer before Stop. The host owns link resolution, scope enforcement on calls, idempotency, and appending the reply through `calternal-chat.send`; that path remains under verification. |
| Format 2.20 | Session scope is resolved by the host and fixed to the current chat session at start. Every host route call made by a run must recheck the same scope and session. |
| Format 7.4; coordinator set 11 | Send-and-start is one Chat call under one request key. Chat send includes the chosen harness, appends the message, and starts it with the append receipt's host-issued canonical link. The harness operation accepts that link as its first actor input. The Chat fixture still needs the coordinator-specified wiring; it was not edited. |
| Format 2.9; coordinator set 11 | Unread state is a record anchor: count ledger records after `state.read through` in ledger order. The Chat fixture still types and compares this value as a timestamp; its owner must update it. |
| Coordinator set 11; revision 9 slots | A slot gets typed inputs from its one typed operation-input declaration, with no optional extras or negotiation. Fixture declarations still need this wiring; they were not edited. |
| Coordinator set 12; lane C frames | Trusted slot selection admits one declared operation; append receipts carry a canonical link; change selectors are discovered once at install; oversized frames are refused; installed code stays off outside tests until OS sandboxing; workers recycle when the actor changes. This run service consumes the declared `s.run` call. |
| Egress decision | The container uses `--network=none`. A per-run read-only socket reaches a host CONNECT proxy that accepts only declared hostnames on port 443, resolves every request itself, rejects unsafe answers, and dials the checked numeric address. The injected fake-test resolver redirects the allowed hostname to a local server. Tunnel logs carry hostname and byte counts only. |
| Format program binding | This harness declares an executable. The runner explicitly rejects image bindings; the format's exact image artifact/loading semantics are not defined by this slice's test vector. Keep that gap visible rather than interpreting an image path as an executable or container tag. |
| Stop boundary | The host authorizer permits Stop only for a caller who can read every fixed target. The wrapper kills the container process family, and the machine reports Exit after the exec ends. The fake ignores SIGINT and has a child; the test checks Stop completes within two seconds. |
| Coordinator set 13 | Append tagging, developer-only local installs, and retention do not change this slice; durability and request receipts remain with the host records implementation. |

## Fixture lines for their owner

- `plugins-v2/calternal-chat/operations.ts`: include the chosen harness and start it with the new record's canonical link under the same request key.
- `plugins-v2/calternal-chat-claude-code/operations.ts`: accept the prompting record link and send it as the run's first actor input.
- `plugins-v2/calternal-chat/kinds/chats.md`: make read-through an anchor and count ledger records after it in order.
- Slot declarations: reference the single typed operation-input declaration.

## Verification and hot-path counts

All commands below ran on `claude@calternal-dev-01-compute02`, in `~/Developer/lanes/runs-harness`, through `~/bin/lock-shared nix develop -c ...`.

- `cargo test -p calternal-machine`: passed; 34 unit tests, 1 address test, 3 egress tests, 1 harness-assignment test, 4 worker tests, 6 Podman tests, 1 exec-environment test, 3 run-service tests and 1 harness test. Two native/performance tests were ignored by their existing requirements.
- `cargo fmt --check -p calternal-machine`: passed.
- `cargo doc --locked -p calternal-machine --no-deps --all-features`: passed.
- The run-service tests cover an allowed CONNECT reaching a local fake server, direct networking blocked, input from another actor refused, ordered logs, and process-family Stop within two seconds. The proxy unit tests cover an undeclared host and a private DNS answer.

Machine run path storage counts: 0 ledger reads, 0 SQLite reads, 0 durable record flushes. The run service validates the installed executable with two path canonicalizations and one metadata read; it does not append records. Host route read/flush counts will be recorded after its integration gate passes. These are structural counts, not performance measurements.

## Simplification opportunities

- The Chat harness uses an executable binding; image bindings can stay out of this runner until the format defines the plugin-local image artifact and loader.
- The harness wrapper and test image both carry small adapter-launch details. If the host later supplies the executable directly in its plugin bundle image, one copy can be removed.
