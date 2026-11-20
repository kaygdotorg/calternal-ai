import type { ItemSave, ItemViewPage } from "../generated/author-types.js";

const target = { item: "item-1", expected: "rev-1", request_id: "save-1" };
const save: ItemSave = { ...target, patch: { title: "New title" } };
const ref: string = save.item;
void ref;
const protect: ItemSave = { ...target, attempt_discovery: "consent_only" };
void protect;
// @ts-expect-error Missing the required target.
const missingTarget: ItemSave = { patch: {} };
// @ts-expect-error A save must make a change.
const noMutation: ItemSave = target;
// @ts-expect-error The host refuses unknown fields.
const extra: ItemSave = { ...target, patch: {}, title: "Not a save operand" };
void [missingTarget, noMutation, extra];

function pageCursor(page: ItemViewPage): string | undefined {
  if (page.view) return page.older;
  return undefined;
}
pageCursor({ view: { content: { type: "snapshot-text", text: "A message" } }, older: "page-2" });
pageCursor({ anchor_unavailable: true });
// @ts-expect-error The reply needs a view or a typed unavailable state.
pageCursor({});
// @ts-expect-error A private older cursor is a string.
pageCursor({ view: { content: { type: "snapshot-text", text: "A message" } }, older: 2 });
