// The numeric limits a plugin author can hit, each read from one pointer in
// the pinned contract. The generator emits them as exported SDK constants; a
// missing or non-integer pointer fails the SDK build instead of guessing.

/** Resolve a JSON pointer inside a parsed contract document. */
function at(document, pointer) {
  return pointer.split("/").slice(1).reduce((value, part) => value?.[part.replaceAll("~1", "/").replaceAll("~0", "~")], document);
}

// Standard padded base64 limits are stated in characters. The catalogue text
// states the decoded budget in whole KiB, which is the largest whole KiB that
// fits the character bound.
const base64Bytes = characters => Math.floor(Math.floor(characters / 4) * 3 / 1024) * 1024;

const table = [
  ["ITEM_INLINE_BODY_BYTES", "catalogue", "/$defs/ItemSave/properties/body/maxLength", "bytes", base64Bytes,
    "Largest body, in decoded bytes, that item_create or item_save accepts inline; stage anything larger."],
  ["ITEM_STAGED_BODY_BYTES", "catalogue", "/$defs/ItemStageBegin/properties/length/maximum", "bytes", undefined,
    "Largest whole item body, in bytes, a staged save accepts; this is also the size bound of one item."],
  ["ITEM_STAGE_CHUNK_BYTES", "catalogue", "/$defs/ItemStageChunk/properties/bytes/maxLength", "bytes", base64Bytes,
    "Largest decoded chunk one item_save_chunk call carries."],
  ["ITEM_APPEND_BYTES", "catalogue", "/file_append/max_decoded_bytes", "bytes", undefined,
    "Largest decoded addition one item_append call commits, including attested framing, quote and middle."],
  ["ITEM_RANGE_BYTES", "catalogue", "/$defs/ItemRange/properties/length/maximum", "bytes", undefined,
    "Largest length one item_range read returns."],
  ["ITEM_SPANS_PER_PAGE", "catalogue", "/$defs/ItemSpansPage/properties/spans/maxItems", "spans", undefined,
    "Most recorded spans one item_spans page returns; follow `next` for the rest."],
  ["ATTESTED_FRAMING_CHARACTERS", "catalogue", "/$defs/AttestedBody/properties/prefix/maxLength", "characters", undefined,
    "Longest attested prefix or suffix; the SDK also refuses framing over this many UTF-8 bytes."],
  ["ITEM_METADATA_FIELDS", "catalogue", "/$defs/ItemCreate/properties/metadata/maxProperties", "fields", undefined,
    "Most metadata fields one item may carry."],
  ["ITEM_FOLDERS", "catalogue", "/$defs/ItemCreate/properties/folders/maxItems", "folders", undefined,
    "Most parent folder components an item_create may name."],
  ["QUOTE_BYTES", "catalogue", "/item_engine/quotes/limits/bytes_per_quote", "bytes", undefined,
    "Largest emitted quote block one attested record may carry."],
  ["QUOTE_COPIES_PER_SOURCE", "catalogue", "/item_engine/quotes/limits/copies_per_source", "copies", undefined,
    "Most live quote copies of one source record across all destinations."],
  ["CONTRIBUTION_FRAME_BYTES", "catalogue", "/extension_points/calls/limits/encoded_frame_bytes", "bytes", undefined,
    "Largest encoded contribution call, entry invoke or output frame, counting JSON plus one LF."],
  ["CONTRIBUTION_READ_FRAMES", "catalogue", "/$defs/ContributionReadResult/properties/frames/maxItems", "frames", undefined,
    "Most frames one contribution_read returns."],
  ["CONTRIBUTION_READ_WAIT_MS", "catalogue", "/$defs/ContributionRead/properties/wait_ms/maximum", "milliseconds", undefined,
    "Longest wait one contribution_read may request."],
  ["JOB_INPUT_FIELDS", "catalogue", "/$defs/ExecutionJobStart/properties/input/maxProperties", "fields", undefined,
    "Most fields in the input a job or alarm passes to its run operation."],
  ["ALARM_INPUT_BYTES", "catalogue", "/alarms/limits/input_bytes", "bytes", undefined,
    "Largest encoded alarm input."],
  ["ALARMS_PENDING_PER_PACKAGE", "catalogue", "/alarms/limits/pending_per_package_per_space", "alarms", undefined,
    "Most pending alarms one package may hold in one space."],
  ["ALARM_GRACE_SECONDS_MAX", "catalogue", "/$defs/AlarmSet/properties/grace_seconds/maximum", "seconds", undefined,
    "Longest grace period an alarm may declare before its delivery counts as missed."],
  ["ALARM_GRACE_SECONDS_DEFAULT", "catalogue", "/$defs/AlarmSet/properties/grace_seconds/default", "seconds", undefined,
    "Grace period the host applies when an alarm omits one."],
  ["VIEW_SNAPSHOT_BYTES", "catalogue", "/view_snapshots/bounds/max_encoded_bytes", "bytes", undefined,
    "Largest encoded item view page a worker may publish or return."],
  ["VIEW_SNAPSHOT_NODES", "catalogue", "/view_snapshots/bounds/max_nodes", "nodes", undefined,
    "Most nodes in one item view page."],
  ["VIEW_SNAPSHOT_DEPTH", "catalogue", "/view_snapshots/bounds/max_depth", "levels", undefined,
    "Deepest nesting of one item view page."],
  ["SNAPSHOT_TEXT_CHARACTERS", "description", "/$defs/SnapshotText/properties/text/maxLength", "characters", undefined,
    "Longest text in one snapshot-text node; the SDK also refuses text over this many UTF-8 bytes."],
  ["VIEWER_FIELDS", "catalogue", "/$defs/ViewerItemState/properties/fields/maxProperties", "fields", undefined,
    "Most viewer-private fields one item may hold for one account."],
  ["REF_CHARACTERS", "catalogue", "/$defs/Ref/maxLength", "characters", undefined,
    "Longest opaque reference, including request IDs and subeffect keys."],
  ["OPERATIONS_PER_PACKAGE", "manifest", "/properties/operations/maxItems", "operations", undefined,
    "Most operations one manifest may declare."],
  ["METHODS_PER_OPERATION", "manifest", "/definitions/operation/properties/methods/maxItems", "methods", undefined,
    "Most broker methods one operation may declare."],
];

/**
 * Read every author limit from the parsed catalogue, description and manifest
 * schemas. Throws naming the pointer when the contract no longer has it.
 */
export function authorLimits(catalogue, description, manifest) {
  const documents = { catalogue, description, manifest };
  return table.map(([name, document, pointer, unit, convert, doc]) => {
    const raw = at(documents[document], pointer);
    if (!Number.isSafeInteger(raw)) throw new Error(`The pinned ${document} has no integer at ${pointer} for ${name}.`);
    const value = convert ? convert(raw) : raw;
    return { name, value, unit, doc, source: `${document}#${pointer}` };
  });
}
