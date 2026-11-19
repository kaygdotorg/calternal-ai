# Plugin SDK v2 and Reading List authoring, 2026-11-19

[Forgejo issue #84](https://git.kayg.org/kayg/calternal-ai/issues/84)

The SDK derives its manifest and app description types from the contract
schemas. The type generator follows registered references across the
description schema, host catalogue, and manifest v1 placement and capability
definitions. It uses the same schema-to-TypeScript compiler as the web
renderer. Structural types catch authoring mistakes in TypeScript. The
package build validates the emitted files through the pinned verifier.
It publishes the manifest after validation and after the referenced
assets exist.

## Eight concepts before a first list

The guide's complete first list uses eight authoring concepts. They are the
package, app, kind, storage placement, item schema, Bind, list, and sidebar
section. A content list and a sidebar section refer to one Bind. The author
omits the title mapping, own-kind reads, network policy, empty operation list,
window, row title and icon, and standard empty, loading, and error states.
The guide builds the same example it publishes as Markdown.

## Reading List fixture parity

- The package ID is `calternal-reading-list` under the project's naming
  rule. The app and kind IDs match the fixture.
- The template and worker asset records contain actual content hashes and
  relative paths. The sealed manifest uses a fixed template filename and
  a placeholder worker digest.
- The SDK adds loading and error states to all Binds, and empty text and a
  60-row count-free window to search. The other fields, actions and forms
  match the sealed fixtures on `dev`.
- The package declares a two-field Add operation and uses the host's created
  reference for navigation. Mark read declares a first full swipe through the
  host metadata patch. The kind declares `host_create` defaults and the
  blank-item button uses `host.item.create` and `then.route`. Add uses a worker
  operation declaration because it takes title and URL from the form.
- The reminder is an own-space example. Its operation and due schedule are
  declarations, and the viewer filter shapes the list. The bundled executable
  exits with failure before making any effect. Runtime work must enforce
  placement and qualify cross-space reference and schedule delivery.

## Revision 13.2 check

The draft at `539c10b6` accepted an isolated Reading List build with three
author changes. Add `host_create: { defaults: { title: "Untitled" } }` to
`reading-item`. Add a New blank item button invoking `host.item.create`
with `inputs: { kind: { literal: item } }` and
`then: { route: { name: "item" } }`. Remove the search Bind's `mode`
so it uses the new default across all authorized searchable fields. The
generated types, manifest and description validators, and two fixture parity
tests passed on dev02. The lane reapplied those changes when revision
13.18 was sealed on `dev`.

## Proposed revision 14 for the morning

How should the host create a personal reminder from an item open in a shared
space? This choice is needed before enabling the shared-space action. For
example, an Editor reading a team link should be able to save a reminder only
in their own space, then follow the reference subject to their live access to
the team link. The conservative first release can offer reminders in the own
space only, at the cost of omitting this shortcut for shared items. A host
handoff could provide the shortcut by deriving the destination from the live
session, reauthorizing both spaces, and returning a bounded durable reference
that grants no access. That requires new activation, retry and revocation
fixtures. Start with own-space-only enablement and qualify the handoff before
exposing it in a shared view. This package leaves execution disabled.

The renderer and scheduler also need a common representation for a date-only
form value. The fixture pairs a date picker with a date-time schema. For
example, September 12 in Los Angeles needs a scheduling time and zone before
it becomes an instant. Add the renderer value and scheduler interpretation to
the same exercise before enabling delivery. The declaration retains fixture
parity pending that decision.

## Remaining conformance

Worker execution, host no-wake behavior, Undo, reminder privacy and schedule
delivery, refusal/retry, a real/fake broker parity suite, the independent
small-model guide trial, and gallery submission are outside this offline
authoring proof. The public guide documents the checked authoring path.
The existing preview guide worker serves the text. Its old scaffold and
check commands still operate on preview manifests.

## Shared generator and verifier follow-up

`packages/contracts/scripts/schema-to-types.mjs` owns reference resolution
and schema compilation. The SDK selects the manifest and app templates.
The web renderer's replacement calls `generateDescriptionTypes` with the
contract directory URL. It keeps its output file and generated drift check.
The renderer lane owns that adoption.

The SDK calls the pinned verifier with `--package` on the emitted directory
before publishing its manifest. Package mode checks the manifest schema,
embedded schema declarations, templates, and template pins. It replaced the
former fixture-staging adapter. The SDK also checks host-create default
values against kind metadata schemas and verifies the optional worker pin.
The shared generator translates 2020-12 `prefixItems` into the compiler's
Draft 7 tuple representation so only the first swipe action can enable
full swipe.

The native verifier reads the manifest and each app template once. The
SDK reads the manifest and each pinned template again to return the
checked authored data, and reads the worker once for its pin. A build also
reads the original worker once when emitting it. There are zero explicit
durability flushes and no runtime path.

## Independent author trials

Both trials gave a fresh Haiku 5.5 at low effort only `GUIDE.md` and
`TASK.md`. The author wrote files in one pass without reading the
installed SDK or running commands. The coordinator installed and checked
the output on dev02. The first-attempt authoring gate passed on Trial 2.

| Trial | Kit directory under SDK thread storage | Result |
| --- | --- | --- |
| 1, Habit Tracker | `/home/kayg/.bb/thread-storage/thr_zpaeug44pe/habit-tracker-trial` | Failed TypeScript with four diagnostics. |
| 2, Recipe Box | `/home/kayg/.bb/thread-storage/thr_zpaeug44pe/recipes-trial` | First-attempt pass. Installation and checks exited 0, the verifier accepted the manifest and description, and the checker reported every required feature. The author wrote a 105-line plugin and the worker. |

In Trial 1, the author counted 21 concepts and identified 14 places where
the prose left the exact spelling open. The submission supplied indexed
field names as an array, used `type` for form fields, and declared untyped
actions separately so their `row: "id"` widened to `string`. The task also
limited the requested output to the plugin declaration, leaving out the
worker. Coordinator evidence under
`/home/kayg/.bb/thread-storage/thr_58j59i9gw3` includes
`trial1-author-notes.md`, `trial1-plugin.ts`, and `trial1-errors.txt`.

Trial 2 used two complete, checked examples. The eight-concept first list
came first, followed by a gear list with indexes, form inputs, inline
swipes, operation, worker, search, and package assembly. The author copied
the example's operation risk, idempotency, methods and required capability
and asked for the full allowed values. They also asked if hidden storage
was required and how to choose single-line or multiline input. The
coordinator provided `trial2-plugin.ts` and reported those notes. The
reference table now derives the five enums from the schemas and has an
independent check for missing or stale values. Placement and text input
choices are explained next to it.

### Third trial after the runtime is available

Use a pantry expiry review plugin. A worker operation validates inputs and
saves an expiry date on an authorized item. A replayable job scans a
bounded set of items in batches and persists progress. A declared schedule
runs the review once at a fixed due time and notifies the creator.

Give a fresh author the guide and task alone. Exercise the generated package
against the real runtime with disposable data and a controlled clock.
Require the operation to persist the expected data, a retry to preserve
the same effect, a restarted job to resume from its checkpoint, and the
schedule to deliver once. Revoke access before a later invocation and
confirm the host refuses the effect. Record worker cleanup and the author's
uncertainties alongside the first-attempt outcome.

## Simplification opportunities

A future verifier could return its checked JSON projection so authoring
checks can reuse its parsed manifest and template data. Swift can share the
same schema graph if its generator needs it.
