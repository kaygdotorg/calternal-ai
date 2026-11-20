import { MAX_V2_FRAME_BYTES } from "@calternal/plugin-sdk";
import type { ItemRef, RevisionRef } from "../generated/wire.js";
import { decode, MAX_FILE_BYTES, page, step, type Thread } from "./format.ts";
import type { Context } from "./interface.ts";

/** The installed item kind shared by both conversation shapes. */
export const kind = "threads";
const slot = "body";

/** Encode the full Markdown body within the inline broker frame budget. */
export function inlineBody(body: Buffer): string {
  if (body.length > MAX_V2_FRAME_BYTES / 2) throw new Error("This edit needs staged item save.");
  return body.toString("base64");
}

/** Read and decode one exact file revision using bounded range calls. */
export async function read(ctx: Context, item: ItemRef, revision: RevisionRef) {
  const chunks: Buffer[] = [];
  let offset = 0;
  while (true) {
    const part = await ctx.call("item_range", { item, revision, offset, length: 65536 });
    const chunk = Buffer.from(part.bytes, "base64");
    const end = offset + chunk.length;
    if (chunk.length > 65536 || end > MAX_FILE_BYTES || (part.next_offset !== undefined && (end <= offset || end !== part.next_offset))) {
      throw new Error("Invalid or oversized thread range.");
    }
    chunks.push(chunk);
    if (part.next_offset === undefined) {
      return { thread: decode(new TextDecoder("utf-8", { fatal: true }).decode(Buffer.concat(chunks))), size: end };
    }
    offset = part.next_offset;
  }
}

/** Publish a page only under a writer scope for the just-read revision. */
export async function publish(ctx: Context, item: ItemRef, revision: RevisionRef, thread: Thread) {
  const scope = await ctx.call("item_view_context", { item, slot });
  if (scope.source_revision !== revision) return;
  await ctx.call("item_view_publish", {
    request_id: step(ctx.author.request_id, `snapshot-${revision}-${scope.audience_scope}`),
    item, slot, ...scope, ...page(thread),
  });
}
