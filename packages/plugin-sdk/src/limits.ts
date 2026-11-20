/**
 * Every numeric limit a plugin author can hit, generated from the pinned
 * contract, and preflight checks that name the limit and the offending value
 * before a broker call is made.
 * @module
 */
import { LIMIT_SOURCES, type LimitName } from "./generated/limits.js";

/** The generated limits, their names and their contract sources. */
export { LIMITS, LIMIT_SOURCES } from "./generated/limits.js";
/** Types of the generated limits table. */
export type { LimitName, Limits, LimitSource } from "./generated/limits.js";

/**
 * A value over a contract limit, raised by an SDK preflight before any
 * effect. `limit` is the exported constant name, `value` the measured size.
 */
export class LimitError extends RangeError {
  /** Stable code shared by every preflight refusal. */
  readonly code = "limit_exceeded";
  /** The limit constant that refused the value. */
  readonly limit: LimitName;
  /** The measured value. */
  readonly value: number;
  /** Create a refusal naming the limit, its bound and the measured value. */
  constructor(limit: LimitName, value: number, what: string) {
    const { value: bound, unit, source } = LIMIT_SOURCES[limit];
    super(`${what} is ${value} ${unit}; ${limit} allows ${bound} (${source}).`);
    this.name = "LimitError";
    this.limit = limit;
    this.value = value;
  }
}

const encoder = new TextEncoder();

/** UTF-8 byte length of a string. Cost: O(length), one allocation. */
export function utf8Length(text: string): number {
  return encoder.encode(text).byteLength;
}

/**
 * Refuse `value` over the named limit. `what` names the measured thing in the
 * message, for example "The reply". Cost: O(1).
 */
export function checkLimit(limit: LimitName, value: number, what: string): void {
  if (value > LIMIT_SOURCES[limit].value) throw new LimitError(limit, value, what);
}

/**
 * Encode a contribution call, entry invoke or output frame as JSON plus LF and
 * refuse it over CONTRIBUTION_FRAME_BYTES, the bound the host applies before
 * launching the entry. Cost: one serialization and UTF-8 encode.
 */
export function encodeContributionFrame(frame: unknown): string {
  const json = JSON.stringify(frame);
  if (json === undefined) throw new TypeError("Expected a JSON contribution frame.");
  const encoded = json + "\n";
  checkLimit("CONTRIBUTION_FRAME_BYTES", utf8Length(encoded), "The encoded contribution frame");
  return encoded;
}

type Node = { type?: unknown; children?: unknown; text?: unknown };

/**
 * Check an item view page against the snapshot byte, node and depth bounds
 * and each snapshot-text node against its character and byte bound. Cost:
 * one serialization plus one walk of the tree.
 */
export function checkItemView(view: { content: unknown }): void {
  checkLimit("VIEW_SNAPSHOT_BYTES", utf8Length(JSON.stringify(view)), "The item view page");
  let nodes = 0;
  const walk = (node: Node, depth: number): void => {
    checkLimit("VIEW_SNAPSHOT_DEPTH", depth, "The item view nesting");
    checkLimit("VIEW_SNAPSHOT_NODES", ++nodes, "The item view node count");
    if (node.type === "snapshot-text" && typeof node.text === "string") {
      checkLimit("SNAPSHOT_TEXT_CHARACTERS", [...node.text].length, "A snapshot-text node");
      checkLimit("SNAPSHOT_TEXT_CHARACTERS", utf8Length(node.text), "A snapshot-text node, in UTF-8 bytes,");
    }
    if (Array.isArray(node.children)) for (const child of node.children) walk(child as Node, depth + 1);
  };
  walk(view.content as Node, 1);
}
