/**
 * Helpers built only from existing broker methods. Each one takes a context
 * whose operation declares the methods it uses, so a missing declaration is a
 * compile error. They add no authority: the host checks every call.
 * @module
 */
import { createHash } from "node:crypto";
import type {
  AttestedBody, ItemRef, ItemSave, ItemSaveReply,
  ItemSpansPage, ItemViewPage, ItemViewPublishReceipt, RevisionRef,
} from "../generated/author-types.js";
import { checkItemView, checkLimit, encodeContributionFrame, LIMITS, utf8Length } from "./limits.js";
import type { OperationContext } from "./operation.js";

const { ATTESTED_FRAMING_CHARACTERS, CONTRIBUTION_READ_WAIT_MS, ITEM_APPEND_BYTES, ITEM_INLINE_BODY_BYTES, ITEM_RANGE_BYTES, ITEM_SPANS_PER_PAGE, ITEM_STAGE_CHUNK_BYTES, ITEM_STAGED_BODY_BYTES } = LIMITS;

const toBase64 = (bytes: Uint8Array) => Buffer.from(bytes.buffer, bytes.byteOffset, bytes.byteLength).toString("base64");

/** A broker reply or state the SDK refuses to trust, such as a range that does not advance. */
export class BrokerReplyError extends Error {
  /** Create a refusal with a caller-safe message. */
  constructor(message: string) {
    super(message);
    this.name = "BrokerReplyError";
  }
}

/**
 * Read the whole body of one exact revision with as many `item_range` calls
 * as it needs (one per ITEM_RANGE_BYTES). Refuses a range that does not
 * advance or a body over ITEM_STAGED_BODY_BYTES. Cost: ceil(bytes / 64 KiB)
 * round trips and one copy of the body.
 */
export async function readBody(ctx: OperationContext<"item_range">, item: ItemRef, revision: RevisionRef): Promise<Buffer> {
  const chunks: Buffer[] = [];
  let offset = 0;
  while (true) {
    const part = await ctx.call("item_range", { item, revision, offset, length: ITEM_RANGE_BYTES });
    const chunk = Buffer.from(part.bytes, "base64");
    const end = offset + chunk.length;
    checkLimit("ITEM_RANGE_BYTES", chunk.length, "One item_range reply");
    checkLimit("ITEM_STAGED_BODY_BYTES", end, "The item body");
    chunks.push(chunk);
    if (part.next_offset === undefined) return Buffer.concat(chunks, end);
    if (end <= offset || part.next_offset !== end) throw new BrokerReplyError("The host returned an item range that does not continue the body.");
    offset = end;
  }
}

/** The recorded spans of one revision, joined across pages. */
export type SpanIndex = Pick<ItemSpansPage, "body_provenance" | "spans">;

/**
 * Read every recorded span of one exact revision, following `next` cursors
 * and joining a span whose host-written ranges continue on the next page
 * (`host_ranges_more`). Refuses pages for another item or revision, repeated
 * cursors, changed provenance and more spans or host ranges than body bytes.
 * Cost: one round trip per ITEM_SPANS_PER_PAGE spans.
 */
export async function readSpans(ctx: OperationContext<"item_spans">, item: ItemRef, revision: RevisionRef, bodyBytes: number): Promise<SpanIndex> {
  const spans: ItemSpansPage["spans"] = [];
  let provenance: ItemSpansPage["body_provenance"] | undefined;
  let cursor: string | undefined;
  let hostRanges = 0;
  const seen = new Set<string>();
  const invalid = () => new BrokerReplyError("The saved message ranges are invalid.");
  do {
    const page = await ctx.call("item_spans", { item, revision, ...(cursor && { cursor }) });
    for (const span of page.spans) hostRanges += span.host_ranges?.length ?? 0;
    if (page.item !== item || page.revision !== revision || (provenance !== undefined && page.body_provenance !== provenance) ||
        page.spans.length > ITEM_SPANS_PER_PAGE || (page.next && (!page.spans.length || seen.has(page.next))) ||
        spans.length + page.spans.length > bodyBytes || hostRanges > bodyBytes) throw invalid();
    const last = page.spans.at(-1);
    if (page.spans.slice(0, -1).some(span => span.host_ranges_more) || (last?.host_ranges_more && !page.next)) throw invalid();
    const prior = spans.at(-1);
    const [first, ...rest] = page.spans;
    if (prior?.host_ranges_more) {
      const same = first?.host_ranges?.length && prior.host_ranges && first.span === prior.span &&
        first.offset === prior.offset && first.length === prior.length && first.middle_offset === prior.middle_offset &&
        first.middle_length === prior.middle_length && JSON.stringify(first.attribution) === JSON.stringify(prior.attribution) &&
        JSON.stringify(first.quote) === JSON.stringify(prior.quote);
      if (!same) throw invalid();
      const { host_ranges_more, ...completed } = first!;
      spans[spans.length - 1] = { ...completed, host_ranges: [...prior.host_ranges!, ...first!.host_ranges!] as typeof prior.host_ranges,
        ...(host_ranges_more && { host_ranges_more }) };
      spans.push(...rest);
    } else spans.push(...page.spans);
    provenance = page.body_provenance;
    cursor = page.next;
    if (cursor) seen.add(cursor);
  } while (cursor);
  return { body_provenance: provenance!, spans };
}

/** A whole-body replacement for `saveBody`. */
export type BodySave = Pick<ItemSave, "item" | "expected" | "patch" | "name" | "attempt_discovery"> & {
  /** Subeffect purpose; the request ID is `ctx.key(purpose)`. */
  purpose: string;
  /** Complete new body. */
  body: Uint8Array;
};

/**
 * Replace a body inline when it fits ITEM_INLINE_BODY_BYTES, otherwise stage
 * it in ITEM_STAGE_CHUNK_BYTES chunks and save the sealed stage under the
 * same request ID. Returns the receipt or the host's head conflict. A replayed
 * staged save returns the original receipt without sending chunks again.
 * Cost: one call inline; staged, three plus one per chunk, and one SHA-256.
 */
export async function saveBody(
  ctx: OperationContext<"item_save" | "item_save_begin" | "item_save_chunk" | "item_save_seal">,
  { purpose, body, ...target }: BodySave,
): Promise<ItemSaveReply> {
  checkLimit("ITEM_STAGED_BODY_BYTES", body.byteLength, "The item body");
  const request_id = ctx.key(purpose);
  if (body.byteLength <= ITEM_INLINE_BODY_BYTES) {
    return ctx.call("item_save", { request_id, ...target, body: toBase64(body) });
  }
  const begin = await ctx.call("item_save_begin", {
    request_id, item: target.item, expected: target.expected, length: body.byteLength,
    sha256: createHash("sha256").update(body).digest("hex"),
  });
  if (!("handle" in begin)) return begin;
  let offset = 0;
  while (offset < body.byteLength) {
    const chunk = body.subarray(offset, offset + ITEM_STAGE_CHUNK_BYTES);
    const ack = await ctx.call("item_save_chunk", { handle: begin.handle, offset, bytes: toBase64(chunk) });
    if (ack.next_offset <= offset || ack.next_offset > body.byteLength) {
      throw new BrokerReplyError("The host could not confirm the staged part of the body.");
    }
    offset = ack.next_offset;
  }
  const { staged } = await ctx.call("item_save_seal", { handle: begin.handle });
  return ctx.call("item_save", { request_id, ...target, staged });
}

/** One framed message for `appendMessage`. */
export type MessageAppend = {
  /** Subeffect purpose; the request ID is `ctx.key(purpose)`. */
  purpose: string;
  /** Target append-file item. */
  item: ItemRef;
  /** Exact head the append extends. */
  expected_revision: RevisionRef;
  /** Plugin framing before the middle, at most ATTESTED_FRAMING_CHARACTERS when attested. */
  prefix?: string;
  /** The message middle: captured input or delivered output text when attested. */
  text: string;
  /** Plugin framing after the middle. */
  suffix?: string;
  /** Where an attested middle comes from; omit to write opaque bytes. */
  source?: AttestedBody["source"];
  /** Checked quote the host composes ahead of the middle. */
  quote?: AttestedBody["quote"];
  /** Item-local entry anchor bound to the new span. */
  entry_anchor?: AttestedBody["entry_anchor"];
  /** Opaque bytes to append when the message cannot be attested. Defaults to prefix, text and suffix. */
  opaque?: () => Uint8Array;
  /** A complete replacement body to save instead of an opaque append, for formats that must rewrite earlier bytes. */
  rewrite?: () => Uint8Array;
};

/** The receipt of `appendMessage` and the write path it chose. */
export interface MessageReceipt {
  /** New head revision. */
  readonly revision: RevisionRef;
  /** Body length after the write. */
  readonly end_offset: number;
  /** Committed receipt ref. */
  readonly receipt: string;
  /** Recorded span of an attested write. */
  readonly span?: string;
  /** `attested` (host-composed span), `append` (opaque bytes) or `inline`/`staged` (whole-body rewrite). */
  readonly mode: "attested" | "append" | "inline" | "staged";
}

/**
 * Append one framed message, choosing the strongest write the input allows:
 * an attested append when `source` is given and the framing fits; otherwise a
 * whole-body rewrite when `rewrite` is given (inline or staged by size);
 * otherwise an opaque append. Refuses an addition over ITEM_APPEND_BYTES and
 * a rewrite over ITEM_STAGED_BODY_BYTES before any call. A head conflict on a
 * rewrite throws. Cost: one call, or `saveBody`'s cost for a rewrite.
 *
 * Replays: the request ID is `ctx.key(purpose)`, so a replay with the same
 * input returns the original receipt. A replay that re-read a head this
 * append already moved sends a different `expected_revision` and the host
 * refuses it as a conflicting replay; look for your own committed record
 * (for example by its entry anchor) before appending again.
 */
export async function appendMessage(
  ctx: OperationContext<"item_append" | "item_save" | "item_save_begin" | "item_save_chunk" | "item_save_seal">,
  message: MessageAppend,
): Promise<MessageReceipt> {
  const { purpose, item, expected_revision, prefix = "", text, suffix = "", source, quote, entry_anchor } = message;
  const request_id = ctx.key(purpose);
  const fits = (framing: string) => framing.length <= ATTESTED_FRAMING_CHARACTERS && utf8Length(framing) <= ATTESTED_FRAMING_CHARACTERS;
  if (source && fits(prefix) && fits(suffix)) {
    checkLimit("ITEM_APPEND_BYTES", utf8Length(prefix + text + suffix), "The attested message");
    const attested: AttestedBody = { source, prefix, suffix, ...(quote && { quote }), ...(entry_anchor && { entry_anchor }) };
    return { ...await ctx.call("item_append", { request_id, item, expected_revision, attested }), mode: "attested" };
  }
  if (message.rewrite) {
    const body = message.rewrite();
    const saved = await saveBody(ctx, { purpose, item, expected: expected_revision, body, attempt_discovery: "consent_only" });
    if ("code" in saved) throw new BrokerReplyError("The item changed while this message was being written.");
    return { revision: saved.revision, end_offset: body.byteLength, receipt: saved.receipt, ...(saved.span && { span: saved.span }),
      mode: body.byteLength <= ITEM_INLINE_BODY_BYTES ? "inline" : "staged" };
  }
  const bytes = message.opaque?.() ?? Buffer.from(prefix + text + suffix);
  checkLimit("ITEM_APPEND_BYTES", bytes.byteLength, "The message");
  return {
    ...await ctx.call("item_append", { request_id, item, expected_revision, bytes: toBase64(bytes), attempt_discovery: "consent_only" }),
    mode: "append",
  };
}

/** A contribution that ended with a state other than `ok`. */
export class ContributionEndedError extends Error {
  /** The terminal state. */
  readonly state: "failed" | "stopped";
  /** Create an error naming the terminal state. */
  constructor(state: "failed" | "stopped") {
    super(`The contribution ended with status ${state}.`);
    this.name = "ContributionEndedError";
    this.state = state;
  }
}

/** One delivered contribution output. */
export interface ContributionOutput {
  /** Sequence number from 1, contiguous per call. */
  readonly seq: number;
  /** Output data, validated by the host against both output schemas. */
  readonly data: unknown;
  /** The call that produced it. */
  readonly call: string;
}

/**
 * Call a contribution for the current attempt and yield each output frame in
 * order. Preflights the call against CONTRIBUTION_FRAME_BYTES, polls
 * `contribution_read` with the longest allowed wait, refuses a gap or repeat
 * in `seq`, and finishes on an `ok` terminal; any other terminal throws
 * ContributionEndedError. Returns the call ref so a caller can name the
 * output. Cost: one call plus one read per batch of frames.
 */
export async function* streamContribution(
  ctx: OperationContext<"contribution_call" | "contribution_read">,
  { purpose, execution, contribution, input }: { purpose: string; execution: string; contribution: string; input: unknown },
): AsyncGenerator<ContributionOutput, void> {
  const request = { request_id: ctx.key(purpose), execution, contribution, input };
  encodeContributionFrame(request);
  const { call } = await ctx.call("contribution_call", request);
  let after: number | undefined;
  while (true) {
    const { frames } = await ctx.call("contribution_read", { call, ...(after !== undefined && { after }), wait_ms: CONTRIBUTION_READ_WAIT_MS });
    for (const frame of frames) {
      if (frame.seq !== (after ?? 0) + 1) throw new BrokerReplyError("A contribution frame arrived out of order.");
      after = frame.seq;
      if (frame.kind === "terminal") {
        if (frame.state !== "ok") throw new ContributionEndedError(frame.state);
        return;
      }
      yield { seq: frame.seq, data: frame.data, call };
    }
  }
}

/**
 * Publish an item view page for exactly `revision`. Asks the host for the
 * writer scope first and publishes nothing when the head has moved on.
 * Checks the page against the snapshot bounds before sending. The request ID
 * is keyed by slot, revision and scope, so a replay reuses the receipt.
 * Cost: two calls plus `render`.
 */
export async function publishView(
  ctx: OperationContext<"item_view_context" | "item_view_publish">,
  { item, slot, revision, render }: {
    item: ItemRef; slot: string; revision: RevisionRef;
    render: () => Promise<Extract<ItemViewPage, { view: unknown }>> | Extract<ItemViewPage, { view: unknown }>;
  },
): Promise<ItemViewPublishReceipt | undefined> {
  const scope = await ctx.call("item_view_context", { item, slot });
  if (scope.source_revision !== revision) return undefined;
  const page = await render();
  checkItemView(page.view);
  return ctx.call("item_view_publish", {
    request_id: ctx.key(`view-${slot}-${revision}-${scope.audience_scope}`),
    item, slot, ...scope, view: page.view, ...(page.older && { older: page.older }),
  });
}
