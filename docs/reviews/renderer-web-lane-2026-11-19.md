# Web description renderer lane: report and proposed revisions (2026-11-19)

Lane: Forgejo #81, branch `lane/v2-renderer-web`, package `packages/description`. Reports per step are on the issue; this file holds what the sealed contract cannot say.

## Proposed revision A: one icon catalogue owned by the contract

**Problem.** A description names icons (`icon: "arrow-up"`, `{kindIcon: true}`, a field `map`, a manifest's `icon: "book"`), and nothing defines the set both renderers must draw. The web `@calternal/ui` `ICONS` set and the SF Symbols mapping would drift, and fixtures already use names the web set lacks.

**Proposal.**
1. The contract owns ONE list `icons` (in `contract/host-catalogue-v2.json`, or a sibling `contract/icons-v2.json`): `{ name, web: <ICONS key>, sf: <SF Symbol name> }`. A name is `Id`-shaped (`^[a-z0-9][a-z0-9._:-]{0,63}$`). The web renderer checks its `ICONS` against `web`, the SwiftUI renderer checks `sf`; neither invents a mapping. No shared code.
2. An unknown name is drawn as a visible neutral placeholder in the icon column and is never dropped (the renderer draws exactly what the description declares). The web renderer already does this (`data-icon-missing`).
3. The verifier rejects an unknown icon name in a description, a manifest `icon`, a `map` value and a state `icon`.
4. Revision 13 (owner decision): a leading icon may be a symbol OR an image reference. Proposal: `Icon` gains `{image: <host-issued ImageRef>}`; the host issues the ref with the row (like activation contexts), the renderer shows it at the icon size (small preview), never a client-chosen URL.

**Names the sealed fixtures use today** (descriptions and manifests): `archive`, `arrow-down`, `arrow-up`, `book`, `check`, `claude`, `clock`, `codex`, `edit`, `folder`, `notes`, `plus`, `settings`, `share`, `spark`, `threads`, `trash`, `warning`.

**Not in the web set today:** `arrow-down`, `arrow-up` (the set has `up` and `down`, which draw the same arrows: either rename or map), `book`. Everything else exists.

To agree with the SwiftUI lane (thr_drqwyzub2f) by message: the name list above and the SF Symbol for each.

## Other gaps found

- `Row.marks` of kind `count` with value 0: the contract does not say whether it draws. The renderer follows the system badge convention and draws nothing; proposal: say so.
- `Predicate` in a mark's `when` can only compare literals on the row's own fields (operands from viewer, route or selection are the host's); proposal: the schema should restrict a mark's `when` to literal operands.
- A stack holding two lists (the reading-list fixture) splits the height between them; section 5.0 says one list per scroll region. Proposal: the verifier rejects a stack with more than one `list` child, or the contract says how they share.

## Icon names agreed with the native lane (thr_drqwyzub2f, 2026-11-19)

Used by the fixtures (name = SF Symbol): archive = archivebox; arrow-down = arrow.down; arrow-up = arrow.up; book = book; check = checkmark; claude = sparkle; clock = clock; codex = chevron.left.forwardslash.chevron.right; edit = pencil; folder = folder; notes = note.text; plus = plus; settings = gearshape; share = square.and.arrow.up; spark = sparkles; threads = bubble.left.and.bubble.right; trash = trash; warning = exclamationmark.triangle.

Native already ships (not in fixtures yet): info, key, lock, shield, bell, search, more, close, chevron, back, pulse, terminal, file, files, table, changes, branch, stop, play, person, copy, reply, mail, question, tasks, apps, sidebar, sidebar-right.

Collisions to settle in the contract: `code` vs `codex` (same SF Symbol) so `code = curlybraces`; `send` vs `arrow-up` so `send = paperplane`. Web: `claude` (one star) and `spark` (three) must stay visually distinct. Web renames to match the shared names: `up` becomes `arrow-up`, `down` becomes `arrow-down`; `book` and `reply`, `mail`, `person` are not in the web set yet.

## Gaps the host components rely on (confirmed by the backend lane, 2026-11-19; revision 14 will specify)

- **Composer context key.** Revision 13.15 puts composer contexts in view-open and renew replies but does not say under which key. The renderer's stand-in reads `viewContexts[<component id>]` and uses the component id as `action_id`; both are assumptions, kept in `src/components/Composer.svelte` and `testing/synthetic-source.ts`.
- **Request card content.** `RequestCardResolve` is defined (`card_context`, `request_ref`, `decision`, `lifetime` required for approvals) but nothing reads a card's title and trusted material. The stand-in invents a `request.card` route; the real one is revision 14's.
- **Transcript, composer options, Send:** synthetic stand-ins stay until the host serves them.

## Hot path counts, bounds and review decisions (code review of 6e989eab)

**Reads and flushes.** A window reply is applied once: O(rows in the reply) work, one reactive flush (`tick`) per reply, at most 4 requests a second and 4 in flight per store. A key press in a list is O(1): the id-to-index map answers `indexOf`, nothing is sent while a key repeats, one settled-selection request follows the rest. A held key drawing 5,000 rows costs two window requests (the prefetch). Rows held per store are bounded by `MAX_ROWS` (2,000); beyond it the rows farthest from the view are dropped and asked for again on scroll back.

**Contract points the review raised.**
- *Open on cursor settle.* The contract says the route opens when the selection settles (section 5.0) and says nothing about an `open.invoke` running then. Conservative choice: a route opens on settle; an `open.invoke` runs only on click or Enter. Gap for revision 14: state whether `open.invoke` may be a plugin operation and whether a settled cursor counts as an activation.
- *Title of a confirmation sheet.* The contract says the material is the host's and the description cannot relabel it (section 3.2), but does not title the sheet. Conservative choice: a fixed host title, "Please confirm". Gap for revision 14: say who titles the sheet.
- *Shell actions.* Settled by the contract (description-contract.md section 3.1 and the `footer.apps` entry in 5.0): the schema enumerates the shell actions and the renderer passes to the shell only a well-formed one of `route`, `reload`, `signOut`, `sheet: trash | share`. The one open point, a gap for revision 14, is that a description can draw an account menu, an app switcher and glass panes that imitate host chrome: host chrome needs a host-owned marker.
- *Reduced motion.* Only the skeleton pulse animates in this package; panes and toasts come from `packages/ui`.
