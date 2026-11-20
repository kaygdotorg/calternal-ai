import type { OperationContext } from "@calternal/plugin-sdk";
import type { ItemRef, ViewerItemStateWrite } from "@calternal/plugin-sdk/types";

/** Shared declarations for the viewer index and the Threads state operation. */
export const viewerFields = {
  preview: { type: "Text" },
  unread: { type: "Number", source: "entry_unread" },
  mentions: { type: "Number", source: "entry_mentions" },
  muted: { type: "Boolean" },
  pinned: { type: "Boolean" },
  draft: { type: "Text" },
  unread_mark: { type: "Boolean" },
} as const;

/** Only manually written preferences are accepted here. Reading and drafts are host actions. */
export const viewerPatchSchema = {
  pinned: { type: ["boolean", "null"] },
  muted: { type: ["boolean", "null"] },
  unread_mark: { type: ["boolean", "null"] },
} as const;

type StateInput = { item: ItemRef; pinned?: boolean | null; muted?: boolean | null; unread_mark?: boolean | null };

/** Read or update the authenticated viewer's declared preferences using the host's compare-and-set. */
export async function viewerState({ item, pinned, muted, unread_mark }: StateInput, ctx: OperationContext<"viewer_state_read" | "viewer_state_write">) {
  const prior = await ctx.call("viewer_state_read", { item });
  const patch: ViewerItemStateWrite["patch"] = {};
  if (pinned !== undefined) patch.pinned = pinned;
  if (muted !== undefined) patch.muted = muted;
  if (unread_mark !== undefined) patch.unread_mark = unread_mark;
  if (Object.entries(patch).every(([key, value]) => (prior.fields[key] ?? null) === value)) return prior;
  return ctx.call("viewer_state_write", { item, expected: prior.version, request_id: ctx.key("viewer-state"), patch });
}
