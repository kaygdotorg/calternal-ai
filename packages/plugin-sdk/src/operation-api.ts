/** Author-facing coded operations for the primitives-only plugin host. @module */

/** Host-assigned item UUID. */
export type ItemId = string;
/** Host-assigned person UUID used in person and people fields. */
export type PersonId = string;
/** Link field containing a Markdown item reference. */
export type ItemLink = string;
/** Link field containing a Markdown reference with a record anchor. */
export type RecordLink = string;
/** Named stored fields or fields supplied by a form. */
export type Fields = Record<string, unknown>;

/** Error codes defined by format §8.1. */
export const operationCodes = [
  "not-allowed", "not-found", "invalid", "conflict", "too-large", "limit",
  "unreachable", "needs-sign-in", "stopped", "timeout",
] as const;

/** Stable caller-safe failure supplied by the host or a coded operation. */
export interface OperationFailure {
  /** Category suitable for programmatic decisions. */
  code: typeof operationCodes[number];
  /** A safe human-readable reason. */
  message: string;
  /** A safe next action. */
  next: string;
}

/** A kind query or a stored collection query. */
export type Query =
  | {
      /** Qualified kind name. */
      from: string;
      /** Formula predicate evaluated by the host. */
      where?: string;
      /** Ordered sort expressions. */
      sort?: readonly string[];
      /** Grouping field. */
      group?: string;
      /** Requested aggregation expressions. */
      totals?: readonly string[];
      /** Field projection. */
      fields?: readonly string[];
      /** Opaque cursor from the previous page. */
      cursor?: string;
      /** Page size up to 200. */
      limit?: number;
    }
  | {
      /** Qualified collection name. */
      collection: string;
      /** Opaque cursor from the previous page. */
      cursor?: string;
      /** Page size up to 200. */
      limit?: number;
    };

/** Projected item returned by a query. */
export interface Row {
  /** Item UUID. */
  id: ItemId;
  /** Qualified kind name. */
  kind: string;
  /** Stored fields visible to the acting person. */
  fields: Fields;
  /** Computed fields evaluated for the acting person. */
  computed: Fields;
  /** Host version for a conditional write. */
  version: number;
}

/** One page of rows and totals from the host. */
export interface Page {
  /** Rows in the requested order. */
  rows: Row[];
  /** Computed aggregate results. */
  totals: Fields;
  /** Opaque next-page cursor, or null on the last page. */
  cursor: string | null;
}

/** A visible item and its Markdown and records. */
export interface Item extends Row {
  /** Markdown outside the item frontmatter. */
  body: string;
  /** Records returned by the host, with ledger stamps. */
  records: unknown[];
}

/** Record payload before the host stamps its author, time and anchor. */
export interface RecordBody {
  /** Values of declared record fields. */
  fields?: Fields;
  /** The record's Markdown body. */
  body?: string;
}

/** One write in the atomic change list. Each entry names exactly one change. */
export type Change =
  | {
      /** Create an item with initial values. */
      add: {
        /** Kind in which the item belongs. */
        kind: string;
        /** Initial declared field values. */
        fields: Fields;
        /** Initial Markdown body. */
        body?: string;
      };
    }
  | {
      /** Update fields or Markdown. */
      change: {
        /** Item to update. */
        item: ItemId;
        /** New values for the named fields. */
        fields?: Fields;
        /** Replacement Markdown body. */
        body?: string;
        /** Required current version for a conditional update. */
        base?: number;
      };
    }
  | {
      /** Remove an item. */
      remove: {
        /** Item to remove. */
        item: ItemId;
      };
    }
  | {
      /** Move an item within its kind. */
      move: {
        /** Item to move. */
        item: ItemId;
        /** Destination subfolder within the kind. */
        folder?: string;
        /** Destination filename within the kind. */
        file?: string;
      };
    }
  | {
      /** Append one record and let the host stamp it. */
      "append record": {
        /** Item whose record stream receives the append. */
        item: ItemId;
        /** Unstamped record fields and body. */
        record: RecordBody;
      };
    }
  | {
      /** Change the record identified by its anchor. */
      "change record": {
        /** Item holding the record. */
        item: ItemId;
        /** Anchor in the item's record stream. */
        anchor: string;
        /** Updated declared fields and body. */
        record: RecordBody;
      };
    }
  | {
      /** Remove the record identified by its anchor. */
      "remove record": {
        /** Item holding the record. */
        item: ItemId;
        /** Anchor in the item's record stream. */
        anchor: string;
      };
    };

/** Receipts in change-list order. */
export interface WriteResult {
  /** Each entry describes the changed item and record anchor, where relevant. */
  changes: {
    /** The addressed item. */
    id: ItemId;
    /** The resulting item version. */
    version: number;
    /** The resulting record anchor, where applicable. */
    anchor?: string;
    /** Host-issued record link, when the route supplies one. */
    link?: RecordLink;
  }[];
}

/** Event context attached by the host. The key is stable across replay. */
export interface OperationEvent {
  /** The registration that opened this event. */
  kind: "call" | "change";
  /** Opaque identifier for this logical delivery. */
  key: string;
  /** Addressed item, or null for an itemless operation. */
  item: ItemId | null;
  /** Visible previous item data when available. */
  before?: unknown;
  /** Host request identifier when available. */
  request?: string;
}

/** Handle for a run in the current operation. */
export interface RunHandle {
  /** Host-assigned run ID. */
  id: string;
  /** Status snapshot returned when the run was opened. */
  status: string;
  /** Send input as the run actor, subject to the host check. */
  send(input: unknown): Promise<void>;
  /** Stop the run if the current caller may read the target. */
  stop(): Promise<void>;
}

/** Host calls and context available while handling one event. */
export interface OperationsContext {
  /** Query visible items through the host. */
  query(query: Query): Promise<Page>;
  /** Read one item through the host. */
  read(item: ItemId): Promise<Item>;
  /** Commit or preview a list as one atomic effect. */
  write(changes: readonly Change[], options?: { preview?: boolean }): Promise<WriteResult>;
  /** Get or set declared viewer state. */
  state: {
    /** Read the acting person's current value. */
    get(item: ItemId, key: string): Promise<unknown>;
    /** Store the acting person's value. */
    set(item: ItemId, key: string, value: unknown): Promise<unknown>;
  };
  /** Invoke another plugin's listed operation through the same authority check. */
  call(operation: string, input: unknown): Promise<unknown>;
  /** Start a declared program with a host-checked scope and targets. */
  run(program: string, options: { scope?: "session"; targets: readonly ItemId[] }): Promise<RunHandle>;
  /** Verified actor and current role for this delivery. */
  readonly me: Readonly<{
    /** Verified identity of the person directing this event. */
    id: PersonId;
    /** Current role in this space. */
    role: string;
  }>;
  /** Space in which the plugin is installed. */
  readonly space: Readonly<{
    /** UUID of the space where this operation is installed. */
    id: string;
  }>;
  /** Metadata scoped to this delivery. */
  readonly event: Readonly<OperationEvent>;
}

/** Exported association between a host event selector and an author handler. */
export interface Operation<T> {
  /** Whether this handler receives a call or a committed change. */
  readonly kind: OperationEvent["kind"];
  /** Declared operation or subscribed kind or collection. */
  readonly selector: string;
  /** Author code invoked with the event input and its scoped host API. */
  readonly handler: (input: T, s: OperationsContext) => unknown;
}

/** Register handlers by exporting the returned declaration from operations.ts. */
export const on = {
  /** Bind a declared coded operation to its handler. */
  call<T>(selector: string, handler: Operation<T>["handler"]): Operation<T> {
    return { kind: "call", selector, handler };
  },
  /** React to a committed change for a kind or collection. */
  change<T>(selector: string, handler: Operation<T>["handler"]): Operation<T> {
    return { kind: "change", selector, handler };
  },
};
