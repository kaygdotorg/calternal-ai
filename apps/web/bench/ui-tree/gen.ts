// The plugin's side, written once and used both in the worker and (for the baseline) on the main thread.
import type { Node, Patch } from "./protocol";
export const N = 10_000;
export const rowOf = (i: number) => ({ title: `Row ${i} ${"abcdefghij".repeat(1 + (i % 3))}`, sub: `item ${i} · detail line` });
export const page = (id: string, first: number, end: number): Patch => ({ op: "rows", id, start: first, items: Array.from({ length: end - first }, (_, k) => rowOf(first + k)) });
const win = (n: number): Node => ({ id: `w${n}`, type: "window", props: { title: `Window ${n}`, w: 420 }, children: [
  { id: `w${n}l`, type: "static-list", props: { n: 30 }, children: [] },
  { id: `w${n}c`, type: "button", props: { label: "Send" } },
] });
export const initial = (): Node => ({ id: "root", type: "stack", props: {}, children: [
  { id: "strip", type: "strip", props: {}, children: [win(1), win(2), win(3)] },
  { id: "list", type: "vlist", props: { count: N, label: "Rows" } },
  { id: "msg", type: "text", props: { text: "" } },
] });
export const windowNode = win;
