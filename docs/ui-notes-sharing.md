# Notes sharing in the web app

Lane report: issue #62. Ported from the existing app (`ShareSheet`, `docs/grills/sharing.md` SH1 to SH10); built against fixtures with the `sharing.*` shapes of issue #60.

## What is where

| Piece | File |
| --- | --- |
| Operation shapes, fixture backend, the "who can see this" sentence | `apps/web/src/apps/notes/share/sharing.ts` |
| The sheet (people, groups, links, QR, Copy) | `.../share/ShareSheet.svelte` |
| Which sheet is open, and the one-call hook `openShare(item, title, button)` | `.../share/share.svelte.ts`, `ShareHost.svelte` |
| Shared with me (sidebar section, header avatar) | `.../share/incoming.svelte.ts`, `Sidebar.svelte`, `Notes.svelte` |
| The guest page `/s/<secret>` | `apps/web/src/guest/Guest.svelte`, mounted from `main.ts` before any sign-in |
| QR encoder (ported, no dependency) | `apps/web/src/lib/qr.ts` |
| Avatar (shared) | `packages/ui/src/components/Avatar.svelte` |

## Rules it follows

- Phone: the inset bottom `Sheet`. Desktop: a `Popover` that grows out of the Share pill (the shared menu rule: touch form gets the sheet).
- What the caller may change comes only from `sharing.list` (`can_manage_audiences`, `can_manage_links`). A part they may not change is not drawn as a control; one line says who can.
- Links are view-only in this slice and never listed with their secret: Copy and the QR ask `sharing.link_copy`.
- The Share pill sits left of the three dots; on a note shared with the person, the owner's avatar sits there and opens the same sheet. A note shared as View is read-only in the editor.
- A canvas window's Share button calls `openShare({ kind: "notes", id }, title, button)`; the note's own app (`Notes.svelte`) mounts the `ShareHost`.

## Confirmed by the sharing lane

Named grants `{grant_id, permission, audience, name}`; link grants `{grant_id, permission: "view", label, expires_at}` (labels may repeat, `link_create` needs a non-blank one); `space_access` "view", "edit" or null. The guest read is `GET /s/<secret>` with `Accept: application/json`: a note `{title, markdown}`, a collection `{title, members: [{title, href}]}`, a member `{title, markdown, back}`; `item: {kind, id}` only when the request has a session whose account can open the item without the link; every dead link is a 404; member `href`s are opaque. Still open: the values of the caller's own `permission`, and the answer shape of `sharing.rescue` (assumed `{note: {id, title}}`).

## Also here

- `real.ts`: the real client (`sharing.*` operations and the guest fetch), chosen by `serverMode()`; fixtures stay the default.
- `rescue.ts` and `NotesStore.rescue`: a write refused because access ended sends only the blocks the person changed, with the cached title, to `sharing.rescue`, and says in one line where they went (a stable request id per text, or the queued write's id).
- Multi-select in the Notes sidebar (`lib/select.svelte.ts`, ported from the existing app): ⌘/Ctrl-click, ⇧-click or a row's Select; Share on several notes calls `collections.create` and opens that collection's sheet; one picked note shares as itself. Not ported yet: the two-finger swipe down, trash from the bar.
- `SearchField` (`packages/ui`): the one search field; the sheet uses it.

## Checked

`apps/web/e2e/share.mjs` (in the gate): both device sizes, the sentence, add a person and a group, View/Edit, a link, QR, Copy, remove, Editor and Viewer limits, Shared with me, the owner's avatar, the guest page signed in and out. Phone frames on the iPhone Simulator are on the issue.
