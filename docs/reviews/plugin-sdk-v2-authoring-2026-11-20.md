# Plugin SDK v2 authoring, 2026-11-20

Lane `lane/sdk-v2-authoring` (issue #106), cut from `lane/v2-host-base` at
contract revision 14.44 (3e62ca07). No contract change. Goal: remove what made
the Chat plugin slow and long to write, measured by porting a copy of it.

## What the SDK now has

| Piece | Where | What it replaces in Chat |
| --- | --- | --- |
| In-process v2 fake host | `packages/plugin-sdk/src/fake-host/` (`FakeHost` from `@calternal/plugin-sdk/testing`) | The 1,317-line `test/host.ts` for every contract exchange Chat uses |
| `defineOperation` with schema-typed handler | `src/operation.ts`, `src/schema.ts` | Declaring each input four or five times; hand-written target |
| Typed `ctx.call` from the operation's `methods` | `OperationContext<M>` in `src/operation.ts`, generated broker table | `interface.ts`'s 18-entry method map and `as unknown as` casts |
| Broker helpers | `src/broker-helpers.ts` | Ranged reads, span paging with continued host ranges, inline-or-staged saves, attested-or-opaque appends, contribution polling, head-checked publication |
| Limits and preflight | `LIMITS`, `LIMIT_SOURCES`, `checkLimit`, `encodeContributionFrame`, `checkItemView` | `MAX_V2_FRAME_BYTES / 2`, hard-coded 512, 65536 and 2 MiB |
| One build command | `calternal-plugin build` / `calternal-plugin limits` (`scripts/calternal-plugin.mjs`) | `build.mjs` (git archive, type generation, tsc, esbuild, builder) and `check-shapes.py` |

The generator now names every catalogue and description definition
(`@calternal/plugin-sdk/types`), so Chat no longer generates its own wire
types, and types keyword-free schemas such as `contribution_input` as
`unknown` (an SDK generator gap from the spike report).

### What the fake host proves and does not

It runs every broker call through the pinned catalogue schemas, the
operation's declared methods, kinds, jobs, targets and view slots, request
receipts and an in-memory item and execution engine, and applies the
installer's kind metadata checks. A missing declaration refuses
`403 not_declared` with the manifest path and the change to make.

It does not prove real sessions and tickets, live authority (roles, grants,
sharing, guests, revocation), audiences, quotas and rate limits beyond sizes,
durability and crash recovery, renderer behavior, real harness images,
egress, or the host's exact path normalization.

## Chat port

A copy of both Chat packages is in `packages/plugin-sdk/examples/chat-port/`
(baseline commit f40702ef, unmodified from `lane/threads-ui` 4e513af8; the port
follows it). Both packages build with `calternal-plugin build` against 14.44,
and 24 of 24 ported tests pass in 0.3 s (build plus tests 1.4 s) on dev-02.

| Lines | Before | After |
| --- | ---: | ---: |
| Build tooling (`build.mjs`, `check-shapes.py`) | 103 | 0 |
| Test host (`test/host.ts`) | 1,317 | 34 |
| Touched source (`plugin.ts`, `interface.ts`, `worker.ts`, `files.ts`, `viewer.ts`, `harnesses-v1.ts`, both claude-code files) | 603 | 491 |
| All source, both packages | 1,884 | 1,767 |
| Chat and legacy tests | 667 | 543 |
| People and attachment tests | 714 | not ported |

Per file: `plugin.ts` 95 → 46, `interface.ts` 39 → 11, `files.ts` 163 → 96,
`worker.ts` 177 → 220 (it now holds every operation declaration with its
handler; `plugin.ts` + `interface.ts` + `worker.ts` went 311 → 277 with each
input declared once).

Not ported, with the reason:

- `people.test.ts` and `attachments.test.ts` (714 lines) and the parts of the
  old host behind them: they rely on ten fixture ports outside the 14.44
  catalogue (people directory, shared create, consent waits, invitations,
  consultation, private preferences, attachment claims) and on people
  handlers that are not declared operations. A catalogue-driven host refuses
  them by design. `people.ts` still compiles against the SDK context.
- Three rename/link tests: `host.item.rename` with an indexed-link rewrite is
  host-owned and outside this lane.
- The synthetic editor-save test: it calls `item_save` with captured input
  through an operation Chat does not declare.

Diff for the owners: the port commits after f40702ef under
`packages/plugin-sdk/examples/chat-port/`. The interface lane (#99) changes
nothing in the description files; `formField` can derive the composer and
create-prompt fields from the operation schemas. The integration lane (#101)
can take `worker.ts`, `files.ts`, `plugin.ts`, `interface.ts` and the
claude-code worker as they are.

## Other changes outside the SDK

- `crates/server/trusted-plugins.json`: three bundled worker pins refreshed.
  The SDK is now `sideEffects: false`, but esbuild's minified names still
  change when the SDK index graph grows; behavior is identical. The guide's
  pin changes again because its bundled reference names the new command.
- Reading List and the plugin guide build with `calternal-plugin build`; the
  walkthrough and reference name it. `calternal-build-plugin`, the
  `author-context` and `contribution-frame` script exports are removed
  without aliases.

## Review checklist

1. Shared types: generated once; Chat's own generated copy is gone.
2. Dependencies: `esbuild` and `typescript` move into the SDK for the build command.
3. Nothing global on a request path: the schema cache is per test process.
4. Blocking calls: only in the CLI (`tsc`, the Python verifier) and at fake-host schema load.
5. Hot path counts: `readBody` one call per 64 KiB; `readSpans` one per 128 spans; `saveBody` one call inline, three plus one per 64 KiB staged; `appendMessage` one; `publishView` two; `streamContribution` one plus one read per batch.
6. Validators compile once and are cached by key.
7. Nothing stored: fake-host state lives for one test.
8. No compatibility code or aliases.
9. One fake space by UUID; no names.
10. Broker methods are joined once (`brokerMethods`), limits read once from contract pointers.
11. Every public item has a non-test caller, except fake-host test hooks (`markRead`, `failNext`) and `formField`, which the interface lane (#99) would call.
12. Reused: the pinned verifier, `verifyPackage`, Ajv, Chat's own span-merge and staging logic moved into helpers.
13. Docs gate passes with no warnings; reference pages regenerated.
14. README, guide walkthrough and reference updated.
15. Largest new file is 470 lines.

## A v2 dev server with hot reload (not built)

It would need from the host: a local single-space host process that installs
a package from a directory without a gallery signature (development trust
mode), a reinstall that keeps items and receipts across package
generations, a renderer session against that host, and a watch hook to
rebuild and reinstall on change. On the SDK side it would wrap
`calternal-plugin build` in a watcher and print the same summary. The fake
host cannot stand in: it has no renderer, sessions or durable state.

## Open questions for the coordinator

- Replay after a moved head: a worker replaying an append keyed by purpose
  re-reads the head and sends a different `expected_revision`, which the host
  refuses as a conflicting replay. Chat dedupes by message ID. Should the
  contract return the committed receipt for the same subeffect instead?
- Should `examples/chat-port/` stay as a regression check for the SDK after
  the owners adopt the diff, or be deleted?
- Python remains a build-time requirement through the pinned verifier.

Contract gaps went to #78 in one comment: 22 broker methods without a pinned
exchange, limits that exist only in prose, and the framing bound's unit.

## Simplification opportunities

- `typedHandlers` and `WorkerRuntimeV2` are the v1-manifest path; once the v2
  carrier adapter uses `authorContext`, they and `manifest-v1.ts` can go.
- `LIMIT_SOURCES` strings add about 3 KB to every worker bundle; a worker
  needs only `LIMITS`, so the sources could move to a separate module.
- Chat's `attest()` and `appendMessage` both decide whether framing fits;
  Chat could drop `attest` and pass the source straight through.
- `people.ts` cannot be tested on a contract-driven host until its operations
  are declared; declaring them (with the description-next shapes) would let
  its 626-line test file shrink the way `chat.test.ts` did.
- The verifier already checks the manifest schema and templates in package
  mode, so the SDK's separate Ajv manifest validator in `verify-package.mjs`
  is now used only for create defaults and could fold into the verifier.
