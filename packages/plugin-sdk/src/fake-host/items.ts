/**
 * The fake host's item engine: items, revisions, recorded spans, attested
 * composition, staged saves, viewer-private state and published item views.
 * In memory, deterministic, one space. No durability or crash recovery.
 */
import { createHash } from "node:crypto";
import type {
  AttestedBody, ItemAppend, ItemAppendReceipt, ItemContentReceipt, ItemCreate, ItemQuoteSource, ItemRangeResult, ItemRecordedSpan,
  ItemSave, ItemSaveReply, ItemSpansPage, ItemStageBegin, ItemStageBeginReply, ItemStageChunkAck, ItemStageSealed, ItemViewContext,
  ItemViewPublish, ItemViewPublishReceipt, ManifestV2, OwnedItemHead, ViewerItemState, ViewerItemStateWrite,
} from "../../generated/author-types.js";
import { checkItemView, LIMITS } from "../limits.js";
import { ensure, notDeclared } from "./refusal.js";
import type { Invocation } from "./invocation.js";

const { ITEM_APPEND_BYTES, ITEM_INLINE_BODY_BYTES, ITEM_RANGE_BYTES, ITEM_STAGED_BODY_BYTES, QUOTE_BYTES, QUOTE_COPIES_PER_SOURCE } = LIMITS;

type Kind = ManifestV2["kinds"][number];
type Origin = NonNullable<ItemRecordedSpan["attribution"]>;
type Binding = { record: string; anchor?: string; ordinal?: number; quoted?: { record: string; source: ItemQuoteSource } };

/** One stored item. Exposed read-only to tests through `FakeHost.item`. */
export interface StoredItem {
  /** Item ref. */
  readonly id: string;
  /** Owning package ID. */
  readonly package: string;
  /** Kind declaration from the owning manifest. */
  readonly kind: Kind;
  /** Account that created the item. */
  readonly creator: string;
  /** Readable path inside the space, from the kind's placement. */
  path: string;
  /** Current metadata. */
  metadata: Record<string, unknown>;
  /** Current head revision. */
  head: string;
  /** Every revision's bytes, oldest first. */
  readonly revisions: Map<string, Buffer>;
  /** The span index committed with each revision. */
  readonly index: Map<string, Pick<ItemSpansPage, "body_provenance" | "spans">>;
  /** Live recorded spans, ordered by offset. */
  spans: ItemRecordedSpan[];
  /** Record identity, entry anchor and quote binding per live span. */
  readonly bindings: Map<string, Binding>;
  /** Whether every byte of the head is explained by recorded spans. */
  provenance: "attested" | "unproven";
  /** Whether attempts started by the creator may discover the item without consent. */
  discovery: "creator_only" | "consent_only";
  /** Account the host attributes the head write to, when it was attested. */
  author?: string;
  /** Archived items refuse writes and job admission. */
  archived: boolean;
  /** Next entry ordinal. */
  entries: number;
}

/** Bytes composed for one attested write. */
type Composed = { bytes: Buffer; prefix: number; quote: number; middle: number; attribution?: Origin; quoted?: Binding["quoted"] & { origin: Origin } };

const quoteBlock = (middle: Buffer) => middle.toString().split("\n").map(line => `> ${line}`).join("\n") + "\n\n";

/** Item engine state and the broker methods over it. */
export class Items {
  /** Items by ref. */
  readonly items = new Map<string, StoredItem>();
  /** Published item views by `item/slot`. */
  readonly views = new Map<string, ItemViewPublish>();
  /** Recorded span page size; lower it to exercise paging. */
  spanPageSize = 128;
  /** Mention lookup for `entry_mentions` viewer fields: accounts mentioned by one entry. */
  mentions: (item: string, anchor: string) => readonly string[] = () => [];
  private stages = new Map<string, { begin: ItemStageBegin; chunks: Buffer[]; length: number; sealed?: Buffer; package: string }>();
  private scopes = new Map<string, { item: string; slot: string; revision: string }>();
  private cursors = new Map<string, { item: string; revision: string; actor: string; after: number }>();
  private viewers = new Map<string, { version: string; fields: Record<string, string | number | boolean>; read?: number }>();
  private quoteCopies = new Map<string, number>();

  /** Create an engine using the host's ID source, clock and space ref. */
  constructor(private id: (prefix: string) => string, private now: () => string, private space: string) {}

  /** Look up an item the invocation's package may read. */
  readable(inv: Invocation, item: string): StoredItem {
    const stored = this.items.get(item);
    ensure(stored && (stored.package === inv.pkg.manifest.id || inv.pkg.manifest.reads?.includes(stored.kind.id)), 404, "not_found", "Item unavailable.");
    return stored;
  }

  /** Look up an item the invocation's package owns and may write now. */
  writable(inv: Invocation, item: string): StoredItem {
    const stored = this.readable(inv, item);
    ensure(stored.package === inv.pkg.manifest.id, 403, "not_owner", "Only the owning package can write this item.");
    ensure(!stored.archived, 409, "target_unavailable", "The target is archived (target unavailable).");
    if (inv.job) ensure(inv.job.item === item, 403, "wrong_target", "An attempt can write only its own target item.");
    return stored;
  }

  /** `item_head`: the current head, size and metadata. */
  head(stored: StoredItem): OwnedItemHead {
    return { item: stored.id, revision: stored.head, locator: { space: this.space, id: stored.id },
      bytes: stored.revisions.get(stored.head)!.length, metadata: structuredClone(stored.metadata) };
  }

  private commit(stored: StoredItem, body: Buffer, opaque: boolean): string {
    const revision = this.id("rev");
    if (opaque) {
      stored.provenance = "unproven";
      stored.spans = [];
      stored.bindings.clear();
    }
    stored.revisions.set(revision, body);
    stored.head = revision;
    stored.spans.sort((a, b) => a.offset - b.offset);
    stored.index.set(revision, { body_provenance: stored.provenance, spans: structuredClone(stored.spans) });
    return revision;
  }

  private attribute(inv: Invocation, stored: StoredItem, attested: AttestedBody | undefined, discovery?: string) {
    stored.author = attested && attested.source.kind !== "empty" ? inv.actor : undefined;
    if (!attested || inv.actor !== stored.creator || discovery === "consent_only") stored.discovery = "consent_only";
  }

  private compose(inv: Invocation, stored: StoredItem | undefined, attested: AttestedBody, replacing?: string): Composed {
    const { source, prefix = "", suffix = "", quote } = attested;
    let middle = "";
    let attribution: Origin | undefined;
    if (source.kind === "input") {
      ensure(inv.captured, 403, "missing_capture", "An input-attested write needs a host-captured submission: invoke with captured input and declare write_origins.host_input.");
      middle = inv.captured.text;
      attribution = { kind: "person", actor: inv.actor };
    } else if (source.kind === "output") {
      const field = inv.operation.write_origins?.attempt_output;
      if (!field) throw notDeclared("An output-attested write", `operations[${inv.index}].write_origins.attempt_output`, "Name the contribution output field the write copies.");
      const frame = inv.job?.attempt && inv.job.outputs.get(source.seq);
      ensure(frame && inv.job?.attempt, 403, "missing_output", `No delivered output ${source.seq} for this attempt.`);
      const value = (frame as Record<string, unknown>)[field];
      ensure(typeof value === "string", 400, "invalid_output", `Output field ${field} is not a string.`);
      middle = value;
      attribution = { kind: "attempt", attempt: inv.job.attempt.execution };
    }
    let quoted: Composed["quoted"];
    let block = "";
    if (quote) {
      const origin = this.items.get(quote.item);
      const span = origin?.index.get(quote.revision)?.spans.find(entry => entry.span === quote.span);
      ensure(origin && origin.package === inv.pkg.manifest.id && span && span.middle_length > 0 && span.attribution && origin.bindings.has(quote.span),
        403, "quote_unavailable", "The quoted message is unavailable.");
      const record = origin.bindings.get(quote.span)!.record;
      ensure((this.quoteCopies.get(record) ?? 0) < QUOTE_COPIES_PER_SOURCE, 409, "quote_copy_limit", `A message can be quoted at most ${QUOTE_COPIES_PER_SOURCE} times.`);
      block = quoteBlock(origin.revisions.get(quote.revision)!.subarray(span.middle_offset, span.middle_offset + span.middle_length));
      ensure(Buffer.byteLength(block) <= QUOTE_BYTES, 413, "quote_limit", `The quote exceeds ${QUOTE_BYTES} bytes.`);
      quoted = { record, source: quote, origin: span.attribution };
    } else if (stored && replacing && source.kind !== "empty") {
      // A replace without a quote keeps the checked block byte for byte.
      const prior = stored.spans.find(span => span.span === replacing);
      const binding = stored.bindings.get(replacing);
      if (prior?.quote && binding?.quoted) {
        block = stored.revisions.get(stored.head)!.subarray(prior.quote.offset, prior.quote.offset + prior.quote.length).toString();
        quoted = { ...binding.quoted, origin: prior.quote.attribution };
      }
    }
    const bytes = Buffer.from(prefix + block + middle + suffix);
    return { bytes, prefix: Buffer.byteLength(prefix), quote: Buffer.byteLength(block), middle: Buffer.byteLength(middle), attribution, quoted };
  }

  private record(stored: StoredItem, offset: number, composed: Composed, anchor?: string, record?: string): string | undefined {
    if (!composed.bytes.length) return undefined;
    const span = this.id("span");
    stored.spans.push({
      span, offset, length: composed.bytes.length, middle_offset: offset + composed.prefix + composed.quote, middle_length: composed.middle,
      ...(composed.middle > 0 && composed.attribution && { attribution: composed.attribution }),
      ...(composed.quoted && { quote: { offset: offset + composed.prefix, length: composed.quote, attribution: composed.quoted.origin } }),
    });
    const prior = record ? [...stored.bindings.values()].find(binding => binding.record === record) : undefined;
    if (anchor) {
      ensure(![...stored.bindings.values()].some(binding => binding.anchor === anchor), 409, "anchor_exists", `Entry anchor ${anchor} already exists.`);
    }
    const entry = prior?.anchor ? { anchor: prior.anchor, ordinal: prior.ordinal } : anchor ? { anchor, ordinal: ++stored.entries } : {};
    stored.bindings.set(span, { record: record ?? span, ...entry, ...(composed.quoted && { quoted: { record: composed.quoted.record, source: composed.quoted.source } }) });
    if (composed.quoted) this.quoteCopies.set(composed.quoted.record, (this.quoteCopies.get(composed.quoted.record) ?? 0) + 1);
    return span;
  }

  private placement(kind: Kind, folders: readonly string[], name: string, title: string): string {
    const storage = kind.storage as { folder?: string; hidden?: boolean } | undefined;
    const root = storage?.hidden ? `.calternal/${kind.id}` : storage?.folder ?? kind.id;
    const parents = [...folders];
    // Approximates the ordinary readable stem; the host's normalization is authoritative.
    if (kind.file_format?.title_binding?.placement === "utc_date_folder") {
      let stem = "";
      for (const character of title.replace(/[^\p{L}\p{N} -]/gu, "_")) {
        if (Buffer.byteLength(stem + character) > 105) break;
        stem += character;
      }
      parents.push(`${this.now().slice(0, 10)} ${stem}`);
    }
    const used = new Set([...this.items.values()].map(item => item.path));
    let path = [root, ...parents, name].join("/");
    for (let suffix = 2; used.has(path); suffix++) {
      path = parents.length ? [root, ...parents.slice(0, -1), `${parents.at(-1)} (${suffix})`, name].join("/") : [root, `${suffix} ${name}`].join("/");
    }
    return path;
  }

  /** `item_create`: place, check title binding and metadata, compose attested bytes. */
  create(inv: Invocation, input: ItemCreate, checkMetadata: (kind: Kind, metadata: unknown) => void): ItemContentReceipt {
    const index = inv.pkg.manifest.kinds.findIndex(kind => kind.id === input.kind);
    if (index < 0) throw notDeclared(`Kind ${input.kind}`, "kinds", `Declare kind ${input.kind} in this package, or write only kinds it owns.`);
    const kind = inv.pkg.manifest.kinds[index]!;
    checkMetadata(kind, input.metadata);
    const composed = input.attested && this.compose(inv, undefined, input.attested);
    const body = composed ? composed.bytes : Buffer.from(input.body ?? "", "base64");
    ensure(body.length <= ITEM_INLINE_BODY_BYTES, 413, "body_limit", `An inline body is at most ${ITEM_INLINE_BODY_BYTES} bytes; stage larger bodies.`);
    const title = String(input.metadata[kind.title_field ?? "title"] ?? "");
    const key = kind.file_format?.title_binding?.frontmatter_key;
    if (key && body.length) {
      const text = body.toString();
      const close = text.indexOf("\n---\n", 3);
      const lines = text.startsWith("---\n") && close > 0 ? text.slice(4, close).split("\n").filter(line => line.startsWith(`${key}:`)) : [];
      ensure(lines.length === 1 && lines[0] === `${key}: ${title}`, 400, "title_binding", `The body must carry exactly one frontmatter line "${key}: <title>" matching the title metadata.`);
    }
    if (input.attempt_discovery === "creator_only" && kind.attempt_discovery !== "item_policy") {
      throw notDeclared("Creator-only discovery", `kinds[${index}].attempt_discovery`, 'Set attempt_discovery to "item_policy" on the kind.');
    }
    const stored: StoredItem = {
      id: this.id("item"), package: inv.pkg.manifest.id, kind, creator: inv.actor,
      path: this.placement(kind, input.folders ?? [], input.name, title), metadata: structuredClone(input.metadata),
      head: "", revisions: new Map(), index: new Map(), spans: [], bindings: new Map(), provenance: "attested",
      discovery: input.attempt_discovery === "creator_only" && inv.captured !== undefined && (input.attested || !body.length) ? "creator_only" : "consent_only",
      archived: false, entries: 0,
    };
    const span = composed && this.record(stored, 0, composed, input.attested?.entry_anchor);
    if (input.attested) stored.author = input.attested.source.kind !== "empty" ? inv.actor : undefined;
    const revision = this.commit(stored, body, !input.attested && body.length > 0);
    this.items.set(stored.id, stored);
    return { item: stored.id, revision, locator: { space: this.space, id: stored.id }, receipt: input.request_id, ...(span && { span }) };
  }

  /** `item_range`: one bounded slice of an exact revision. */
  range(inv: Invocation, { item, revision, offset, length }: { item: string; revision: string; offset: number; length: number }): ItemRangeResult {
    const body = this.readable(inv, item).revisions.get(revision);
    ensure(body, 404, "revision_unavailable", "That revision is unavailable.");
    const page = body.subarray(offset, offset + Math.min(length, ITEM_RANGE_BYTES));
    const next = offset + page.length;
    return { bytes: page.toString("base64"), ...(next < body.length && { next_offset: next }) };
  }

  /** `item_spans`: paged recorded spans with cursors pinned to item, revision and actor. */
  spansPage(inv: Invocation, { item, revision, cursor }: { item: string; revision: string; cursor?: string }): ItemSpansPage {
    const index = this.readable(inv, item).index.get(revision);
    ensure(index, 404, "revision_unavailable", "That revision is unavailable.");
    let after = 0;
    if (cursor) {
      const pin = this.cursors.get(cursor);
      ensure(pin && pin.item === item && pin.revision === revision && pin.actor === inv.actor, 400, "invalid_cursor", "That span cursor belongs to another read.");
      after = pin.after;
    }
    const spans = index.spans.slice(after, after + this.spanPageSize);
    const next = after + spans.length < index.spans.length ? this.id("spans-page") : undefined;
    if (next) this.cursors.set(next, { item, revision, actor: inv.actor, after: after + spans.length });
    return { item, revision, body_provenance: index.body_provenance, spans: structuredClone(spans), ...(next && { next }) };
  }

  /** `item_append`: attested or opaque addition under the exact expected head. */
  append(inv: Invocation, input: ItemAppend): ItemAppendReceipt {
    const stored = this.writable(inv, input.item);
    ensure(input.expected_revision === stored.head, 409, "stale_head", "The item changed; read the new head and append again.");
    const prior = stored.revisions.get(stored.head)!;
    const composed = input.attested && this.compose(inv, stored, input.attested);
    const addition = composed ? composed.bytes : Buffer.from(input.bytes ?? "", "base64");
    ensure(addition.length <= ITEM_APPEND_BYTES, 413, "append_limit", `One append is at most ${ITEM_APPEND_BYTES} bytes.`);
    ensure(prior.length + addition.length <= ITEM_STAGED_BODY_BYTES, 413, "item_limit", `An item is at most ${ITEM_STAGED_BODY_BYTES} bytes.`);
    const span = composed && this.record(stored, prior.length, composed, input.attested?.entry_anchor);
    if (!input.attested) stored.provenance = "unproven";
    this.attribute(inv, stored, input.attested, input.attempt_discovery);
    const body = Buffer.concat([prior, addition]);
    const revision = this.commit(stored, body, false);
    return { revision, end_offset: body.length, receipt: input.request_id, ...(span && { span }) };
  }

  /** `item_save_begin`: reserve a staged replacement under the expected head. */
  begin(inv: Invocation, input: ItemStageBegin): ItemStageBeginReply {
    const stored = this.writable(inv, input.item);
    if (input.expected !== stored.head) return { status: 409, code: "head_conflict", current: this.head(stored) };
    const handle = this.id("stage");
    this.stages.set(handle, { begin: input, chunks: [], length: 0, package: inv.pkg.manifest.id });
    return { handle };
  }

  /** `item_save_chunk`: the next contiguous chunk; an identical resend is acknowledged. */
  chunk(inv: Invocation, { handle, offset, bytes }: { handle: string; offset: number; bytes: string }): ItemStageChunkAck {
    const stage = this.stages.get(handle);
    ensure(stage && stage.package === inv.pkg.manifest.id && !stage.sealed && offset <= stage.length, 404, "stage_unavailable", "That staged save is unavailable.");
    const chunk = Buffer.from(bytes, "base64");
    ensure(offset + chunk.length <= stage.begin.length, 400, "stage_overflow", "The chunk runs past the declared length.");
    if (offset < stage.length) {
      ensure(Buffer.concat(stage.chunks).subarray(offset, offset + chunk.length).equals(chunk), 409, "stage_conflict", "A resent chunk differs.");
      return { next_offset: stage.length };
    }
    stage.chunks.push(chunk);
    stage.length += chunk.length;
    return { next_offset: stage.length };
  }

  /** `item_save_seal`: check length and digest. */
  seal(inv: Invocation, { handle }: { handle: string }): ItemStageSealed {
    const stage = this.stages.get(handle);
    ensure(stage && stage.package === inv.pkg.manifest.id && stage.length === stage.begin.length, 409, "stage_incomplete", "The staged body is incomplete.");
    const body = Buffer.concat(stage.chunks);
    ensure(createHash("sha256").update(body).digest("hex") === stage.begin.sha256, 409, "stage_digest", "The staged body does not match its SHA-256.");
    stage.sealed = body;
    return { staged: handle };
  }

  /** `item_save`: inline, staged, attested, span replacement, metadata patch. */
  save(inv: Invocation, input: ItemSave, checkMetadata: (kind: Kind, metadata: unknown) => void): ItemSaveReply {
    const stored = this.writable(inv, input.item);
    if (input.expected !== stored.head) return { status: 409, code: "head_conflict", current: this.head(stored) };
    const stage = input.staged === undefined ? undefined : this.stages.get(input.staged);
    if (input.staged !== undefined) {
      ensure(stage?.sealed && stage.begin.item === input.item && stage.begin.expected === input.expected && stage.begin.request_id === input.request_id,
        409, "stage_mismatch", "The staged body belongs to another save.");
    }
    const current = stored.revisions.get(stored.head)!;
    let body = stage?.sealed ?? (input.body === undefined ? current : Buffer.from(input.body, "base64"));
    ensure(body.length <= ITEM_STAGED_BODY_BYTES, 413, "item_limit", `An item is at most ${ITEM_STAGED_BODY_BYTES} bytes.`);
    let span: string | undefined;
    const opaque = Boolean(stage) || input.body !== undefined;
    if (opaque) {
      const quoted = stored.spans.some(entry => stored.bindings.get(entry.span)?.quoted);
      ensure(!quoted, 409, "quote_tracking", "A whole-body change cannot discard a checked quote.");
    }
    if (input.attested && input.replace_span) {
      const prior = stored.spans.find(entry => entry.span === input.replace_span);
      ensure(prior, 404, "span_unavailable", "That span is not live at the head.");
      const composed = this.compose(inv, stored, input.attested, input.replace_span);
      const { record } = stored.bindings.get(prior.span)!;
      body = Buffer.concat([current.subarray(0, prior.offset), composed.bytes, current.subarray(prior.offset + prior.length)]);
      const delta = composed.bytes.length - prior.length;
      stored.spans = stored.spans.filter(entry => entry.span !== prior.span);
      stored.bindings.delete(prior.span);
      for (const entry of stored.spans) if (entry.offset > prior.offset) {
        entry.offset += delta;
        entry.middle_offset += delta;
        if (entry.quote) entry.quote.offset += delta;
        for (const range of entry.host_ranges ?? []) range.offset += delta;
      }
      span = this.record(stored, prior.offset, composed, undefined, record);
    } else if (input.attested) {
      const composed = this.compose(inv, stored, input.attested);
      body = composed.bytes;
      stored.spans = [];
      stored.bindings.clear();
      stored.provenance = "attested";
      span = this.record(stored, 0, composed);
    }
    if (input.patch) {
      const metadata = { ...stored.metadata, ...input.patch };
      checkMetadata(stored.kind, metadata);
      stored.metadata = metadata;
    }
    if (opaque || input.attested) this.attribute(inv, stored, input.attested, input.attempt_discovery);
    else if (input.attempt_discovery === "consent_only") stored.discovery = "consent_only";
    if (stage) this.stages.delete(input.staged!);
    const revision = this.commit(stored, body, opaque);
    return { item: stored.id, revision, locator: { space: this.space, id: stored.id }, receipt: input.request_id, ...(span && { span }) };
  }

  private viewerKey(stored: StoredItem, actor: string) {
    return JSON.stringify([stored.id, stored.package, actor]);
  }

  private viewerDeclaration(inv: Invocation, stored: StoredItem) {
    const index = inv.pkg.manifest.kinds.findIndex(kind => kind.id === stored.kind.id);
    const fields = (inv.pkg.manifest.kinds[index]?.viewer_fields ?? {}) as Record<string, { type: string; source?: string }>;
    if (!Object.keys(fields).length) throw notDeclared("Viewer state", `kinds[${index}].viewer_fields`, "Declare the viewer-private fields on the kind.");
    return fields;
  }

  /** `viewer_state_read`: the actor's private record plus host-maintained entry counts. */
  viewerRead(inv: Invocation, item: string): ViewerItemState {
    const stored = this.readable(inv, item);
    ensure(!inv.job, 403, "viewer_only", "Viewer state belongs to an interactive person, not a job.");
    const declared = this.viewerDeclaration(inv, stored);
    const record = this.viewers.get(this.viewerKey(stored, inv.actor)) ?? { version: "initial", fields: {} };
    const unread = stored.spans.flatMap(span => {
      const binding = stored.bindings.get(span.span);
      const own = span.attribution?.kind === "person" && span.attribution.actor === inv.actor;
      return binding?.anchor && span.middle_length && (binding.ordinal ?? 0) > (record.read ?? 0) && !own ? [binding.anchor] : [];
    });
    const fields: Record<string, string | number | boolean> = { ...record.fields };
    for (const [name, field] of Object.entries(declared)) {
      if (field.source === "entry_unread") fields[name] = unread.length;
      if (field.source === "entry_mentions") fields[name] = unread.filter(anchor => this.mentions(item, anchor).includes(inv.actor)).length;
    }
    return { version: record.version, fields };
  }

  /** `viewer_state_write`: compare-and-set of declared, author-writable fields. */
  viewerWrite(inv: Invocation, input: ViewerItemStateWrite): ViewerItemState {
    const stored = this.readable(inv, input.item);
    ensure(!inv.job, 403, "viewer_only", "Viewer state belongs to an interactive person, not a job.");
    const declared = this.viewerDeclaration(inv, stored);
    const key = this.viewerKey(stored, inv.actor);
    const record = this.viewers.get(key) ?? { version: "initial", fields: {} };
    ensure(record.version === input.expected, 409, "version_conflict", "Private state changed; read it again.");
    const fields = { ...record.fields };
    const types: Record<string, string> = { Text: "string", Number: "number", Boolean: "boolean", Timestamp: "string" };
    for (const [name, value] of Object.entries(input.patch)) {
      const field = declared[name];
      ensure(field && !field.source, 400, "field_not_writable", `Viewer field ${name} is undeclared or maintained by the host.`);
      if (value === null) delete fields[name];
      else {
        ensure(typeof value === types[field.type], 400, "field_type", `Viewer field ${name} expects ${field.type}.`);
        fields[name] = value;
      }
    }
    this.viewers.set(key, { ...record, version: this.id("viewer"), fields });
    return this.viewerRead(inv, input.item);
  }

  /** Test hook: record that `actor` has read through `anchor`, as the renderer's read mark would. */
  markRead(item: string, actor: string, anchor: string): void {
    const stored = this.items.get(item);
    const binding = stored && [...stored.bindings.values()].find(entry => entry.anchor === anchor);
    ensure(stored && binding?.ordinal, 404, "not_found", "Entry unavailable.");
    const key = this.viewerKey(stored, actor);
    const record = this.viewers.get(key) ?? { version: "initial", fields: {} };
    if (binding.ordinal > (record.read ?? 0)) this.viewers.set(key, { ...record, version: this.id("viewer"), read: binding.ordinal });
  }

  /** `item_view_context`: a writer scope for the current head of a declared slot. */
  viewContext(inv: Invocation, { item, slot }: { item: string; slot: string }): ItemViewContext {
    const stored = this.writable(inv, item);
    const index = inv.pkg.manifest.kinds.findIndex(kind => kind.id === stored.kind.id);
    if (!stored.kind.view_snapshots?.some(entry => entry.id === slot)) {
      throw notDeclared(`View slot ${slot}`, `kinds[${index}].view_snapshots`, `Add { id: "${slot}", refresh: "<read operation>" } to the kind.`);
    }
    const audience_scope = this.id("scope");
    this.scopes.set(audience_scope, { item, slot, revision: stored.head });
    return { audience_scope, source_revision: stored.head };
  }

  /** `item_view_publish`: store one page for the exact head the scope was issued for. */
  publish(inv: Invocation, input: ItemViewPublish): ItemViewPublishReceipt {
    const stored = this.writable(inv, input.item);
    const scope = this.scopes.get(input.audience_scope);
    ensure(scope && scope.item === input.item && scope.slot === input.slot && scope.revision === input.source_revision, 403, "scope_mismatch", "That view scope was issued for another item, slot or revision.");
    ensure(input.source_revision === stored.head, 409, "stale_snapshot", "The item changed since the scope was issued.");
    checkItemView(input.view);
    this.views.set(`${input.item}/${input.slot}`, structuredClone(input));
    return { source_revision: stored.head, receipt: input.request_id };
  }
}

