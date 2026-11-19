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
