/** Author-facing coded operations for the primitives-only plugin host. @module */

// BEGIN HOST OPERATION CONTRACT
/** A kind query or a stored collection query accepted by the standalone host. */
export type Query =
  | {
      /** Qualified kind name. */
      "from": string;
      /** Formula predicate evaluated by the host. */
      "where"?: string;
      /** Ordered sort expressions, up to eight for an ad hoc query. */
      "sort"?: readonly string[];
      /** Case-insensitive substring in the kind’s declared search fields. */
      "search"?: string;
      /** Opaque cursor from the previous page. */
      "cursor"?: string;
      /** Page size up to 200. */
      "limit"?: number;
    }
  | {
      /** Qualified collection name. */
      "collection": string;
      /** Case-insensitive substring in the kind’s declared search fields. */
      "search"?: string;
      /** Opaque cursor from the previous page. */
      "cursor"?: string;
      /** Page size up to 200. */
      "limit"?: number;
    };

/** Changes supported by the host's atomic write route. */
export type Change =
  | {
      /** Create an item with initial values. */
      "add": {
        /** Qualified kind in which to create the item. */
        "kind": string;
        /** Declared field values to store. */
        "fields"?: Fields;
        /** Markdown body to store. */
        "body"?: string;
      };
    }
  | {
      /** Update fields or Markdown. */
      "change": {
        /** UUID of the item addressed by the change. */
        "item": ItemId;
        /** Declared field values to store. */
        "fields"?: Fields;
        /** Markdown body to store. */
        "body"?: string;
        /** Required current version for a conditional change. */
        "base"?: number;
      };
    }
  | {
      /** Evaluate a declared item assignment and commit its changes atomically. */
      "perform": {
        /** UUID of the item addressed by the change. */
        "item": ItemId;
        /** Name of the declared operation to evaluate on the current item. */
        "operation": string;
        /** Declared field values to store. */
        "fields"?: Fields;
        /** Required current version for a conditional change. */
        "base"?: number;
      };
    }
  | {
      /** Remove an item. */
      "remove": {
        /** UUID of the item addressed by the change. */
        "item": ItemId;
      };
    }
  | {
      /** Append one record and let the host stamp it. */
      "append record": {
        /** UUID of the item addressed by the change. */
        "item": ItemId;
        /** Unstamped record fields and body. */
        "record": RecordBody;
      };
    }
  | {
      /** Change the record identified by its anchor. */
      "change record": {
        /** UUID of the item addressed by the change. */
        "item": ItemId;
        /** Anchor in the item's record stream. */
        "anchor": string;
        /** Unstamped record fields and body. */
        "record": RecordBody;
      };
    }
  | {
      /** Remove the record identified by its anchor. */
      "remove record": {
        /** UUID of the item addressed by the change. */
        "item": ItemId;
        /** Anchor in the item's record stream. */
        "anchor": string;
      };
    };

/** Change discriminants accepted by the host. */
export const operationChanges = ["add","change","perform","remove","append record","change record","remove record"] as const;

/** Callback wire names accepted by the host. */
export const operationMethods = {
  /** Query rows through the host. */
  query: "s.query",
  /** Read an item through the host. */
  read: "s.read",
  /** Commit or preview an ordered list atomically through the host. */
  write: "s.write",
  /** Get or set viewer state through the host. */
  state: "s.state",
  /** Invoke a declared operation through the host. */
  call: "s.call",
  /** Start a run through the host. */
  run: "s.run",
  /** Send input to a run through the host. */
  runSend: "s.run.send",
  /** Stop a run through the host. */
  runStop: "s.run.stop",
} as const;

/** One callback wire name. */
export type OperationMethod = typeof operationMethods[keyof typeof operationMethods];
// END HOST OPERATION CONTRACT

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
  /** Group keys for this row in a grouped collection. */
  groups?: unknown[];
  /** Host version for a conditional write. */
  version: number;
}

/** One page of rows and totals from the host. */
export interface Page {
  /** Rows in the requested order. */
  rows: Row[];
  /** Overall aggregates in declaration order, or an empty object without aggregates. */
  totals: unknown[] | Record<string, never>;
  /** Group key, matched row count and pre-page aggregates for each visible group. */
  groups: {
    /** Value used by the collection to identify this group. */
    key: unknown;
    /** Number of matching items in this group before pagination. */
    matched: number;
    /** Group aggregates in the collection's declaration order. */
    totals: unknown[];
  }[];
  /** Computed collection header totals keyed by declared name. */
  namedTotals: Fields;
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
  /** Qualified operation resolved from the person's slot selection, when present. */
  selected_operation?: string;
}

/** Handle for a run in the current operation. */
export interface RunHandle {
  /** Host-assigned run ID. */
  id: string;
  /** Status snapshot returned when the run was opened. */
  status: string;
  /** Queue input as the run actor. The result reports admission to the input channel. */
  send(input: unknown): Promise<{
    /** The input entered the run queue. Delivery to the adapter is unconfirmed. */
    status: "queued";
  }>;
  /** Stop the run if the current caller has access to its original scope. */
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
  /** Invoke a declared operation using its plugin.kind.operation identifier. */
  call(operation: string, input: unknown): Promise<unknown>;
  /** Start a declared program in the current session. The host needs an
   * operator-configured run service to admit the program. */
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
