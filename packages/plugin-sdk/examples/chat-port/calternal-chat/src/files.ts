import { appendMessage, checkLimit, publishView, readBody, readSpans, saveBody, type OperationContext } from "@calternal/plugin-sdk";
import type { AttestedBody, ItemRef, ItemSave, RevisionRef } from "@calternal/plugin-sdk/types";
import { decode, decodeIndexed, encode, encodeRecord, MAX_FILE_BYTES, MAX_FRAMING_BYTES, type messageRecord, needsOpaqueEscape, page, quoteBlock, type Thread } from "./format.ts";
import { ports } from "./interface.ts";
import { decodeLegacy, isLegacy } from "./legacy.ts";
import { checkSelection, fileLinks } from "./attachments.ts";

/** The installed item kind shared by both conversation shapes. */
export const kind = "chats";
const slot = "body";

type Read = OperationContext<"item_range" | "item_spans">;
type Save = OperationContext<"item_save" | "item_save_begin" | "item_save_chunk" | "item_save_seal">;

/** Encode the full Markdown body for an inline create. */
export function inlineBody(body: Buffer): string {
  checkLimit("ITEM_INLINE_BODY_BYTES", body.length, "This conversation");
  return body.toString("base64");
}

async function readCurrent(ctx: Read, item: ItemRef, revision: RevisionRef, bytes: Buffer) {
  const { spans, body_provenance } = await readSpans(ctx, item, revision, bytes.length);
  const parsed: ReturnType<typeof decodeIndexed> = spans.length || body_provenance === "attested" ? decodeIndexed(bytes, spans, body_provenance) : {
    thread: decode(new TextDecoder("utf-8", { fatal: true }).decode(bytes)),
    spans: new Map(), quotes: new Set(),
  };
  return { ...parsed, size: bytes.length };
}

/** Load either supported format, with old transcripts available to read handlers only. */
export async function readConversation(ctx: Read, item: ItemRef, revision: RevisionRef) {
  const bytes = await readBody(ctx, item, revision);
  if (isLegacy(bytes)) return { legacy: true as const, thread: decodeLegacy(bytes) };
  return { legacy: false as const, ...await readCurrent(ctx, item, revision, bytes) };
}

/** Read a writable version 2 document at one exact revision. */
export async function read(ctx: Read, item: ItemRef, revision: RevisionRef) {
  const bytes = await readBody(ctx, item, revision);
  if (isLegacy(bytes)) throw new Error("Version 1 chats are read-only in Chat.");
  return readCurrent(ctx, item, revision, bytes);
}

/** Commit an opaque replacement, inline or staged by size, under the subeffect key for `purpose`. */
export async function saveOpaque(ctx: Save, purpose: string, target: Pick<ItemSave, "item" | "expected" | "patch">, body: Buffer) {
  if (body.length > MAX_FILE_BYTES) throw new Error("Thread is full.");
  const saved = await saveBody(ctx, { purpose, ...target, body, attempt_discovery: "consent_only" });
  if ("code" in saved) throw new Error(saved.code);
  return saved;
}

/** Resolve staged files with the host without exposing their bytes to Threads. */
export async function prepareFiles(ctx: object, item: ItemRef, staged: readonly string[]) {
  checkSelection(staged);
  const { attachments } = ports(ctx);
  if (!attachments) throw new Error("File attachments are unavailable.");
  const files = await attachments.prepare(item, staged);
  if (Buffer.byteLength(fileLinks(files)) + 4 > MAX_FRAMING_BYTES) throw new Error("The attachment names are too long for one message.");
  return files;
}

/** Resolve a page's attachment leaves through the current reader or publisher scope. */
export function conversationPage(ctx: object, item: ItemRef, revision: RevisionRef, thread: Thread, older?: string, anchor?: string) {
  const port = ports(ctx).attachments;
  return page(thread, older, anchor, port && (files => port.project(item, revision, files)));
}

/** Append a record, preserving literal text if an opaque write would discard earlier spans. */
export async function appendRecord(ctx: Save & OperationContext<"item_append">, item: ItemRef, revision: RevisionRef, purpose: string,
  size: number, thread: Thread, record: ReturnType<typeof messageRecord>, attested?: AttestedBody, staged?: readonly string[]) {
  const { attachments } = ports(ctx);
  if (staged?.length && (!attachments || !attested)) throw new Error("Use a checked message to attach these files.");
  const bytes = Buffer.from(attested ? record.prefix + quoteBlock(record.quote) + record.text + record.suffix : encodeRecord(record));
  const rewrite = !attested && thread.messages.slice(0, -1).some(message => needsOpaqueEscape(message.text));
  if (!rewrite && size + bytes.length > MAX_FILE_BYTES) throw new Error("Thread is full.");
  if (staged?.length) {
    // Proposed attachment claim (#78): the port appends with the same checked operand.
    return attachments!.append({ request_id: ctx.key(purpose), item, expected_revision: revision,
      attested: { ...attested!, ...(record.anchor && { entry_anchor: record.anchor }) } }, staged);
  }
  return appendMessage(ctx, {
    purpose, item, expected_revision: revision, prefix: record.prefix, text: record.text, suffix: record.suffix,
    ...(attested && { source: attested.source, ...(attested.quote && { quote: attested.quote }), ...(record.anchor && { entry_anchor: record.anchor }) }),
    opaque: () => Buffer.from(encodeRecord(record)),
    ...(rewrite && { rewrite: () => Buffer.from(encode(thread)) }),
  });
}

/** Publish a page only under a writer scope for the just-read revision. */
export async function publish(ctx: OperationContext<"item_view_context" | "item_view_publish">, item: ItemRef, revision: RevisionRef, thread: Thread) {
  await publishView(ctx, { item, slot, revision, render: async () => {
    const result = await conversationPage(ctx, item, revision, thread);
    if (!("view" in result) || !result.view) throw new Error("The conversation page is unavailable.");
    return { view: result.view, ...(result.older && { older: result.older }) };
  } });
}
