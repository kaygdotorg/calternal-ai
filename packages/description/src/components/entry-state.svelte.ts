/**
 * The live state of one `item-view` component (docs/description-contract.md, entry views): the page the host returned with its reader overlays, the reply or edit mode a message action opened for the composer, and the
 * range selection. One per component id; a composer whose `entry_context` names the component reads it, so a read-only view disables its composer and a reply shows what it replies to.
 */
import { SvelteMap } from "svelte/reactivity";
import type { ItemViewReply, SnapshotEntry, SnapshotNode } from "../draft";

/** The composer mode a checked entry action opened: reply or edit, the entry, the host's composer ref and the edit prefill. */
export interface ComposerMode { mode: "reply" | "edit"; anchor: string; ref: string; initial?: string }

/** State shared between an item view, its entries and the composer bound to it. */
export class ItemViewState {
  /** The newest page and its overlays (null until the first open). */
  reply = $state<ItemViewReply | null>(null);
  /** The reply or edit mode in force, or null. */
  composer = $state<ComposerMode | null>(null);
  /** Whether the person is choosing messages for a range action. */
  selecting = $state(false);
  /** The anchors picked so far (first, then last). */
  picked = $state<string[]>([]);
  /** The entry that has the keyboard's place (roving focus), or null. */
  active = $state<string | null>(null);
  /** Anchors of entries that arrived with the newest page (they ease in once), and the unread separator that is leaving. */
  fresh = $state<ReadonlySet<string>>(new Set());
  leaving = $state<string | null>(null);
  /** The message the range grew from (a click or the key that started it). */
  origin = $state<string | null>(null);
  /** How many messages the picked range covers, for the announcement. */
  count = $state(0);
  /** The host's reference for the picked range, once issued. */
  selectionRef = $state<string | undefined>();

  /** Whether the host says the thread cannot be written in, and why. */
  get readOnly(): { reason: NonNullable<NonNullable<ItemViewReply["access"]>["reason"]> } | null {
    const a = this.reply?.access;
    return a?.state === "read_only" ? { reason: a.reason ?? "grant" } : null;
  }
  /** The host's controls for one entry (own flag, provider mark, offered actions). */
  controlOf(anchor: string) { return this.reply?.entry_controls?.find((c) => c.anchor === anchor); }
  /** The entry node with this anchor on the page, or undefined. */
  entryOf(anchor: string): SnapshotEntry | undefined {
    const walk = (n: SnapshotNode | undefined): SnapshotEntry | undefined => {
      if (!n) return undefined;
      if (n.type === "snapshot-entry") return n.anchor === anchor ? n : undefined;
      if (n.type === "stack") for (const c of n.children) { const f = walk(c); if (f) return f; }
      return undefined;
    };
    return walk(this.reply?.view?.content);
  }
  /** Leaves the selection mode and forgets what was picked. */
  endSelection() { this.selecting = false; this.picked = []; this.selectionRef = undefined; this.count = 0; this.origin = null; }
}

/** The item views of the open app by component id (reactive: a composer drawn before its view still finds it). */
export class ItemViews { readonly map = new SvelteMap<string, ItemViewState>(); }
