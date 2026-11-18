/** The bench's patch protocol: the smallest shape that a plugin tree needs (ids, set, append, insert, remove, rows). */
export type Node = { id: string; type: string; props: Record<string, unknown>; children?: Node[] };
export type Patch =
  | { op: "tree"; root: Node }
  | { op: "set"; id: string; props: Record<string, unknown> }
  | { op: "append"; id: string; text: string }
  | { op: "insert"; parent: string; index: number; node: Node }
  | { op: "remove"; id: string }
  | { op: "count"; id: string; n: number }
  | { op: "rows"; id: string; start: number; items: { title: string; sub: string }[] };
export type ToWorker = { ev: "range"; id: string; first: number; end: number } | { ev: "run"; scenario: string; seq: number } | { ev: "stop" };
export type FromWorker = { patches: Patch[]; sent: number; seq?: number };
