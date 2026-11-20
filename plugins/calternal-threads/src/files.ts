import { createHash } from "node:crypto";
import { isDeepStrictEqual } from "node:util";
import { MAX_V2_FRAME_BYTES } from "@calternal/plugin-sdk";
import type { AttestedBody, ItemRef, ItemSave, ItemSpansPage, RevisionRef } from "../generated/wire.js";
import { decode, decodeIndexed, encode, encodeRecord, MAX_FILE_BYTES, type messageRecord, needsOpaqueEscape, page, quoteBlock, step, type Thread } from "./format.ts";
import type { Context } from "./interface.ts";

/** The installed item kind shared by both conversation shapes. */
export const kind = "threads";
const slot = "body";

function mergeSpanPage(index: ItemSpansPage["spans"], page: ItemSpansPage) {
  const prior = index.at(-1);
  const [first, ...remaining] = page.spans;
  const invalid = () => new Error("The saved message ranges are invalid.");
  if (page.spans.slice(0, -1).some(span => span.host_ranges_more) || (page.spans.at(-1)?.host_ranges_more && !page.next)) throw invalid();
  if (prior?.host_ranges_more) {
    if (!first?.host_ranges?.length || !prior.host_ranges ||
        first.span !== prior.span || first.offset !== prior.offset || first.length !== prior.length ||
        first.middle_offset !== prior.middle_offset || first.middle_length !== prior.middle_length ||
        !isDeepStrictEqual(first.attribution, prior.attribution) ||
        !isDeepStrictEqual(first.quote, prior.quote)) throw invalid();
    const { host_ranges_more, ...completed } = first;
    index[index.length - 1] = { ...completed, host_ranges: prior.host_ranges, ...(host_ranges_more && { host_ranges_more }) };
    prior.host_ranges.push(...first.host_ranges);
    index.push(...remaining);
  } else {
    index.push(...page.spans);
  }
}

/** Encode the full Markdown body within the inline broker frame budget. */
export function inlineBody(body: Buffer): string {
  if (body.length > MAX_V2_FRAME_BYTES / 2) throw new Error("This edit is too large to save in one step.");
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
      throw new Error("The conversation file could not be read within its size limit.");
    }
    chunks.push(chunk);
    if (part.next_offset === undefined) {
      const bytes = Buffer.concat(chunks);
      const index: ItemSpansPage["spans"] = [];
      let provenance: ItemSpansPage["body_provenance"] | undefined;
      let cursor: string | undefined;
      const seen = new Set<string>();
      do {
        const page = await ctx.call("item_spans", { item, revision, ...(cursor && { cursor }) });
        if (page.item !== item || page.revision !== revision ||
            (provenance !== undefined && page.body_provenance !== provenance) ||
            page.spans.length > 128 || (page.next && (!page.spans.length || seen.has(page.next))) ||
            index.length + page.spans.length > bytes.length) throw new Error("The saved message ranges are invalid.");
        mergeSpanPage(index, page);
        provenance = page.body_provenance;
        cursor = page.next;
        if (cursor) seen.add(cursor);
      } while (cursor);
      const parsed: ReturnType<typeof decodeIndexed> = index.length || provenance === "attested" ? decodeIndexed(bytes, index, provenance!) : {
        thread: decode(new TextDecoder("utf-8", { fatal: true }).decode(bytes)),
        spans: new Map(), quotes: new Set(),
      };
      return { ...parsed, size: end };
    }
    offset = part.next_offset;
  }
}

/** Commit an opaque replacement, staging large files before the same checked save. */
export async function saveOpaque(ctx: Context, target: Pick<ItemSave, "request_id" | "item" | "expected" | "patch">, body: Buffer) {
  if (body.length > MAX_FILE_BYTES) throw new Error("Thread is full.");
  let content;
  if (body.length <= MAX_V2_FRAME_BYTES / 2) content = { body: inlineBody(body) };
  else {
    const { item, request_id, expected } = target;
    const begin = await ctx.call("item_save_begin", {
      item, request_id, expected, length: body.length, sha256: createHash("sha256").update(body).digest("hex"),
    });
    if ("code" in begin) throw new Error(begin.code);
    if ("revision" in begin) return begin;
    let offset = 0;
    while (offset < body.length) {
      const chunk = body.subarray(offset, offset + 65536);
      const ack = await ctx.call("item_save_chunk", { handle: begin.handle, offset, bytes: chunk.toString("base64") });
      if (ack.next_offset < offset + chunk.length || ack.next_offset > body.length) {
        throw new Error("The host could not confirm the saved portion of the conversation.");
      }
      offset = ack.next_offset;
    }
    content = await ctx.call("item_save_seal", { handle: begin.handle });
  }
  const saved = await ctx.call("item_save", { ...target, ...content, attempt_discovery: "consent_only" });
  if ("code" in saved) throw new Error(saved.code);
  return saved;
}

/** Append a record, preserving literal text if an opaque write would discard earlier spans. */
export async function appendRecord(ctx: Context, item: ItemRef, revision: RevisionRef, request_id: string, size: number,
  thread: Thread, record: ReturnType<typeof messageRecord>, attested?: AttestedBody) {
  const bytes = Buffer.from(attested ? record.prefix + quoteBlock(record.quote) + record.text + record.suffix : encodeRecord(record));
  if (!attested && thread.messages.slice(0, -1).some(message => needsOpaqueEscape(message.text))) {
    const body = Buffer.from(encode(thread));
    const saved = await saveOpaque(ctx, { request_id, item, expected: revision }, body);
    return { revision: saved.revision, end_offset: body.length };
  }
  if (size + bytes.length > MAX_FILE_BYTES) throw new Error("Thread is full.");
  return ctx.call("item_append", {
    request_id, item, expected_revision: revision, ...(attested ? { attested } : { bytes: bytes.toString("base64"), attempt_discovery: "consent_only" }),
  });
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
