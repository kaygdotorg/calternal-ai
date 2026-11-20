/**
 * Shapes of contract revision 14.41 that the sealed 13.18 schema (and so `generated/model.ts`) does not have yet: the `item-view` host component, the snapshot nodes a checked item view returns, and the extension
 * choice reply. Written by hand from `docs/description-contract.md` sections 5.1 and 5.2 until the sealed schema carries them; the 13.18 shapes stay as they are.
 */

/** One action offered on a message (14.43 `EntryAction`): an invoke, optionally only on the viewer's own entries and for a bounded age. Its context is issued by the host per entry (`entry_controls`). */
export interface EntryAction { invoke: import("./model").Invoke; own_only?: true; max_age_seconds?: number }
/** The `item-view` host component: a read-only, host-checked view of one item's body, live and followed. 14.43 adds range selection, the entry actions and the reading counts it may show. */
export interface ItemViewConfig {
  item: import("./model").ValueSource; slot: string;
  selectionMode?: "single" | "range"; entry_actions?: EntryAction[];
  /** Ids of the declared viewer-private count fields the view shows (`unread`, and `mentions`). */
  reading?: { unread: string; mentions?: string };
}

/** A line of item content, at most 2,000 code points: literal, or with `markup: "inline"` the closed subset (emphasis, strong, code spans, explicit http(s) links). */
export interface SnapshotText { type: "snapshot-text"; anchor?: string; text: string; style?: "title" | "heading" | "body" | "caption"; tone?: "good" | "warn" | "bad"; markup?: "inline" }
/** A checked quote: the text and author come from the host's `quotes` projection for the entry, never from the page. */
export interface SnapshotQuote { type: "snapshot-quote"; source: { item: string; revision: string; span: string } }
/** A link to another item, opened through `item.locate` (an unavailable target is drawn without a title). */
export interface SnapshotLink { type: "snapshot-link"; label: string; target: { space: string; id: string } }
/** A date or system line between entries. */
export type SnapshotMarker = { type: "snapshot-marker"; kind: "date"; timestamp: string } | { type: "snapshot-marker"; kind: "system"; text: string };
/** One message of a conversation: author (name and an opaque grouping key), UTC time, optional edit time and "asked by", queued or failed state, and its content. */
export interface SnapshotEntry {
  type: "snapshot-entry"; anchor: string; timestamp: string; author: { key: string; name: string }; edited_at?: string; asked_by?: string; state?: "queued" | "failed";
  children: (SnapshotText | SnapshotCode | SnapshotImage | SnapshotAttachment | SnapshotQuote | SnapshotLink)[];
}
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
export type SnapshotNode = SnapshotText | SnapshotCode | SnapshotHeading | SnapshotImage | SnapshotAttachment | SnapshotStack | SnapshotEntry | SnapshotMarker;

/** The host's answer to `item_view.open`. */
export interface ItemViewReply {
  state: "live" | "ready" | "retained" | "stale" | "loading" | "unavailable" | "anchor_unavailable";
  source_revision?: string; window_ref?: string; result_version?: string;
  /** Present when the view can be followed: subscribe with `item_view.follow`. */
  follow?: string;
  /** The cursor of earlier content. */
  older?: string;
  view?: { content: SnapshotNode };
  /** The page an anchor or position open returns, and whether the host could not find it. */
  anchor?: string;
  /** Live reader overlays (14.43), present on a live member view only. */
  access?: { state: "read_write" | "read_only"; reason?: "lapsed" | "archived" | "trashed" | "grant" };
  reading?: { unread?: number; mentions?: number; first_unread?: string; next_mention?: string; version: string };
  entry_controls?: { anchor: string; entry: string; own?: true; provider_icon?: string; actions?: { action: string; activation_context: string }[] }[];
  quotes?: { entry: string; source: SnapshotQuote["source"]; text: string; author?: string; target?: { anchor: string; item: { space: string; id: string } } }[];
  requests?: { anchor: string; card: RequestCardRecord }[];
  activity?: { label: string; kind: "typing" | "working" | "consulting"; expires_at: string }[];
}
/** A host request card drawn at an entry (a consent request for the person whose messages were selected). */
export interface RequestCardRecord { request_ref: string; title: string; detail?: string; material: { name: string; label: string; value: unknown }[]; state: "open" | "approved" | "denied" | "expired"; card_context?: string; actions: { decision: "approve" | "deny"; lifetimes?: ("once" | "this_item")[] }[]; expires_at?: string }
/** A follow signal: the content changed (fetch the newest head) or was invalidated (clear it). */
export interface ItemViewSignal { type: "item_view_signal"; follow: string; state: "changed" | "invalidated" | "reader_state" | "closed" }
