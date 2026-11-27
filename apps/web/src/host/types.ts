/**
 * Generic declaration and wire types for the declaration-driven host client.
 * Nothing in this module names any specific plugin or kind.
 */

/** Authenticated browser session returned by the host sign-in route. */
export interface SessionInfo {
  account: string;
  name: string;
  space: string;
}

/** The host's answer for the current browser cookie. */
export interface HostSessionStatus {
  setup: boolean;
  session: SessionInfo | null;
}

/** One field declaration from a kind's `fields:`, `records:`, or `state:` block. */
export interface FieldDecl {
  name: string;
  type: string;
  required?: boolean;
  default?: unknown;
  computed?: boolean;
  formula?: string;
  /** Whether a row can present this numeric field as a badge. */
  badge?: boolean;
}

/** Placement hints on a kind operation (`swipe`, `menu`, `primary`). */
export type OpPlacement = "swipe" | "menu" | "primary";

/** One assignment entry on a `set` operation declaration. */
export interface OperationAssignmentDecl {
  target: string;
  value?: unknown;
  computed?: boolean;
  ask?: boolean;
}

/** One operation declaration from a kind's `operations:` block. */
export interface OperationDecl {
  name: string;
  /** Form: built-in (`add`, `change`, `remove`, `change record`, `remove record`), `set`, or `code`. */
  form: "add" | "change" | "remove" | "change_record" | "remove_record" | "set" | "code" | "duplicate" | "open" | "timer" | "append_record";
  /** Fields accepted by `add`, `change`, or `change record`. */
  fields: string[];
  /** For single-assignment `set` operations: target field or `state.<key>`. */
  target?: string;
  /** For single-assignment `set` operations: literal value or `"ask"`. */
  value?: unknown;
  /** Full assignment list for single- or multi-field `set` operations. */
  assignments?: OperationAssignmentDecl[];
  /** Declared coded arguments, keyed by the exact input name. */
  inputs: Record<string, FieldDecl>;
  /** Visibility on a particular row is supplied by its server-evaluated `when` map. */
  when?: string;
  /** Optional compiled edit window. */
  within?: { months: number; seconds: number };
  /** Where the operation appears in UI surfaces. */
  placement: OpPlacement[];
}

/** One entry in a kind's `page:` declaration. */
export interface PageEntryDecl {
  kind: "field" | "body" | "properties" | "records";
  name?: string;
  recordFields?: string[];
}

/** Parsed kind declaration (`kinds/<id>.md`). */
export interface KindDecl {
  id: string;
  qualified: string;
  plugin: string;
  name: string;
  folder: string;
  editable: "outside" | "operations";
  fields: Record<string, FieldDecl>;
  records: Record<string, FieldDecl>;
  state: Record<string, FieldDecl>;
  row: string[];
  page: PageEntryDecl[];
  operations: Record<string, OperationDecl>;
}

/** Installed collection view with ordered column expressions or view arguments. */
export interface CollectionViewDecl {
  type: "list" | "table" | "board" | "gallery" | "calendar" | "timeline"
    | "chart" | "chart bar" | "chart line" | "map" | "folders" | "compact" | "strip";
  columns: string[];
}

/** Parsed collection declaration (`collections/<id>.md`). */
export interface CollectionDecl {
  id: string;
  qualified: string;
  plugin: string;
  title: string;
  /** Default icon token for links to this collection. */
  icon?: string | null;
  from: string;
  where?: string;
  sort: string[];
  views: CollectionViewDecl[];
}

/** One entry in a sidebar file (`sidebars/<id>.md`). */
export interface SidebarEntryDecl {
  type: "collection" | "section";
  title: string;
  collection?: string;
  /** Icon token declared on this link, which takes precedence over the collection icon. */
  icon?: string | null;
}

/** Parsed sidebar declaration (`sidebars/<id>.md`). */
export interface SidebarDecl {
  id: string;
  plugin: string;
  title: string;
  entries: SidebarEntryDecl[];
}

/** Input names from the slot operation's checked schema. */
export interface ComposerBindingsDecl {
  /** Text input in the operation schema. */
  text: string;
  /** Optional state key on the target kind, without the state prefix. */
  draft: string | null;
  /** Optional record-link input in the operation schema. */
  quote: string | null;
}

/** Slot declaration from `plugin.md`. */
export interface SlotDecl {
  name: string;
  /** Display label declared by the slot owner. */
  label?: string | null;
  cardinality: "single" | "many";
  gives: string;
  recommended: string | null;
  pickedBy: "space" | "person";
  inputOperation: string | null;
  /** Checked bindings used to compose text and optional draft and quote values. */
  composer?: ComposerBindingsDecl | null;
  fillers: (Omit<FillDecl, "slot"> & { plugin: string })[];
}

/** Fill declaration from `plugin.md`. */
export interface FillDecl {
  slot: string;
  kind: "operation" | "frame" | "collection";
  target: string;
  /** Declared label or the supplying plugin's display name. */
  label?: string;
}

/** Parsed plugin declaration bundle returned by the host. */
export interface PluginDecl {
  id: string;
  name: string;
  icon: string;
  version: string;
  description: string;
  slots: Record<string, SlotDecl>;
  fills: Record<string, FillDecl>;
  kinds: Record<string, KindDecl>;
  collections: Record<string, CollectionDecl>;
  sidebars: Record<string, SidebarDecl>;
}

/** Full declarations payload returned by the host for a space. */
export interface SpaceDeclarations {
  space: string;
  plugins: PluginDecl[];
}

/** One item row returned by `s.query`. */
export interface RowData {
  id: string;
  kind: string;
  fields: Record<string, unknown>;
  computed: Record<string, unknown>;
  state?: Record<string, unknown>;
  /** The host has evaluated each named operation precondition for this row. */
  when?: Record<string, boolean>;
  version: number;
  edited?: number;
}

/** One record inside an item returned by `s.read` or pushed over the live socket. */
export interface RecordData {
  anchor: string;
  author: string;
  author_name?: string;
  at: string;
  body: string;
  fields: Record<string, unknown>;
  edited?: boolean;
  redacted?: boolean;
  /** Run UUID stamped on records written by an agent run. */
  run?: string;
  /** Optional in-progress streaming indicator (from run logs). */
  streaming?: boolean;
}

/** Full item returned by `s.read`. */
export interface ItemDetail {
  id: string;
  kind: string;
  fields: Record<string, unknown>;
  computed: Record<string, unknown>;
  state?: Record<string, unknown>;
  /** The host has evaluated each named operation precondition for this row. */
  when?: Record<string, boolean>;
  body: string;
  records: RecordData[];
  version: number;
}

/** Caller-safe host error shape (format §8.1). */
export interface HostError {
  code: string;
  message: string;
  next: string;
}

/** Query subscription payload for `s.query` and live `subscribe`. */
export interface QuerySpec {
  collection?: string;
  from?: string;
  where?: string;
  sort?: string[];
  limit?: number;
}

/** Viewer-aware metadata sent on records snapshots and item changes. */
export interface LiveItemMetadata {
  version: number;
  fields: Record<string, unknown>;
  computed: Record<string, unknown>;
  state: Record<string, unknown>;
  when: Record<string, boolean>;
}

/** One chunk delivered in a `run_log` live frame. */
export type RunLogChunk =
  | { offset: number; event: "data"; stream: "stdout" | "stderr"; bytes: string }
  | { offset: number; event: "exit"; code: number | null; signal?: string | null };

/** Live run log batch frame from `GET /v1/spaces/{space}/live`. */
export interface RunLogFrame {
  type: "run_log";
  sub: string;
  run: string;
  offset: number;
  chunks: RunLogChunk[];
  stopped: boolean;
}

/** Server-to-client frame on `GET /v1/spaces/{space}/live`. */
export type LiveServerFrame =
  | { type: "ready"; account: string; space: string }
  | { type: "rows"; sub: string; rows: RowData[]; totals: Record<string, unknown>; cursor: string | null }
  | (LiveItemMetadata & {
      type: "records";
      sub: string;
      item: string;
      mode: "snapshot" | "delta";
      after: string | null;
      records: RecordData[];
      last_anchor: string | null;
    })
  | { type: "record_append"; sub: string; item: string; version: number; record: RecordData }
  | (LiveItemMetadata & { type: "item_change"; sub: string; item: string })
  | RunLogFrame
  | { type: "error"; sub: string | null; code: string; message: string; next: string }
  | { type: "pong" };
