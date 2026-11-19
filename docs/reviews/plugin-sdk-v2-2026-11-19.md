# Plugin SDK v2 and Reading List authoring, 2026-11-19

[Forgejo issue #84](https://git.kayg.org/kayg/calternal-ai/issues/84)

The SDK derives its manifest and app description types from the contract
schemas. The type generator follows registered references across the
description schema, host catalogue, and manifest v1 placement and capability
definitions. It uses the same schema-to-TypeScript compiler as the web
renderer. Structural types catch authoring mistakes in TypeScript. The
package build checks conditions and bounds against the manifest schema and runs
the original description verifier against each emitted template and its
manifest.

The build pins the serialized template and worker bytes. It publishes the
manifest after validation and after its referenced assets exist. Each template
is serialized and hashed once. An optional worker asset is read and hashed
once. The verifier reads each staged template and corresponding manifest
and loads its schema and catalogue once per build. Build outputs receive
ordinary writes and renames with zero explicit durability flushes. There is no
runtime path, database access, or runtime read or flush count.

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
  host metadata patch. The current sealed catalogue and manifest schema lack
  generic blank-item creation. Proposed revision 13 adds `host_create`
  defaults and `host.item.create`, so the package can add that action after
  the revision lands. Add still needs the worker because it takes title and URL
  values from the form.
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
tests passed on dev02. The checked-in package uses the sealed contract on
`dev` while revision 13.2 is still under review.

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

The SDK checks the emitted files before publishing a manifest. The wrapper
reads the staged manifest, every app template, and the optional worker once.
Each asset is hashed when named and again after staging. The wrapper
presents the description bytes and manifest to the sealed verifier in its
fixture layout. Python reads each paired template and manifest once. The build
has zero explicit durability flushes and no runtime path.

To give the verifier a native package mode in a future numbered revision,
accept a manifest path and the package root, resolve the manifest's selected
templates, and run the same shape and semantic checks. Keep the fixture suite
as the default invocation. This would remove the SDK's temporary fixture
layout and give diagnostics the actual package paths.

## Simplification opportunities

Give the sealed verifier a package input mode so callers can pass artifacts
directly, including a list of installed dependency manifests. Swift can use the same schema graph when its type generator needs it.
