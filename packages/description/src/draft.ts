/**
 * Shapes of contract revision 14.41 that the sealed 13.18 schema (and so `generated/model.ts`) does not have yet: the `item-view` host component, the snapshot nodes a checked item view returns, and the extension
 * choice reply. Written by hand from `docs/description-contract.md` sections 5.1 and 5.2 until the sealed schema carries them; the 13.18 shapes stay as they are.
 */

/** The `item-view` host component: a read-only, host-checked view of one item's body, live and followed. */
export interface ItemViewConfig { item: import("./model").ValueSource; slot: string }

/** A plain line of item content (never markup), at most 2,000 code points. */
export interface SnapshotText { type: "snapshot-text"; anchor?: string; text: string; style?: "title" | "heading" | "body" | "caption"; tone?: "good" | "warn" | "bad" }
/** Literal code, shown verbatim. */
export interface SnapshotCode { type: "snapshot-code"; anchor?: string; text: string; language?: string }
/** A heading with optional authored attribution; the instant is UTC and drawn in the viewer's time zone and locale. */
export interface SnapshotHeading { type: "snapshot-heading"; anchor?: string; title: string; author?: string; timestamp?: string }
/** A checked image, fetched only through the host's asset route. */
export interface SnapshotImage { type: "snapshot-image"; anchor?: string; ref: string; alt?: string }
/** A checked attachment shown with a plain label. */
export interface SnapshotAttachment { type: "snapshot-attachment"; anchor?: string; ref: string; label: string }
/** A column or row of snapshot nodes (at most 24 children, depth 6). */
export interface SnapshotStack { type: "stack"; axis?: "vertical" | "horizontal"; title?: string; children: SnapshotNode[] }
/** What an item view may contain. */
export type SnapshotNode = SnapshotText | SnapshotCode | SnapshotHeading | SnapshotImage | SnapshotAttachment | SnapshotStack;

/** The host's answer to `item_view.open`. */
export interface ItemViewReply {
  state: "live" | "retained" | "stale" | "loading" | "unavailable";
  source_revision?: string; window_ref?: string; result_version?: string;
  /** Present when the view can be followed: subscribe with `item_view.follow`. */
  follow?: string;
  /** The cursor of earlier content. */
  older?: string;
  view?: { content: SnapshotNode };
}
/** A follow signal: the content changed (fetch the newest head) or was invalidated (clear it). */
export interface ItemViewSignal { type: "item_view_signal"; follow: string; state: "changed" | "invalidated" | "closed" }
