import { describe, expect, it } from "vitest";
import { Selection } from "../src";
import type { SelectionContextResult } from "../src";

function host() {
  const calls: { row: string; resolve: (r: Partial<SelectionContextResult>) => void }[] = [];
  const lease = { lease_ref: "l-1", cache_partition: "cp-1", lease_seconds: 15 as const, grant_revision: "g" };
  return { calls, lease, selection: (bind: string, row: string) => new Promise<SelectionContextResult>((res) => calls.push({ row, resolve: (r) => res({ type: "selection_context", lease_ref: "l-1", cache_partition: "cp-1", bind, row, activation_contexts: { open: `c-${row}` }, ...r } as SelectionContextResult) })) };
}

describe("Selection", () => {
  it("paints at once and sends nothing until it settles; then exactly one request", async () => {
    const h = host(); const s = new Selection("notes", h);
    for (const id of ["a", "b", "c", "d"]) s.paint(id);
    expect(s.drawn).toBe("d"); expect(h.calls.length).toBe(0);
    const done = s.settle("d"); h.calls[0]!.resolve({}); await done;
    expect(h.calls.length).toBe(1); expect(h.calls[0]!.row).toBe("d"); expect(s.contextFor("open")).toBe("c-d");
  });
  it("hands out a context only for the row drawn now", async () => {
    const h = host(); const s = new Selection("notes", h);
    const done = s.settle("a"); h.calls[0]!.resolve({}); await done;
    s.paint("b");
    expect(s.contextFor("open")).toBeUndefined(); // the drawn row moved on: wait for the host's context, send nothing
  });
  it("ignores a superseded answer, another lease or partition, and another row", async () => {
    const h = host(); const s = new Selection("notes", h);
    const first = s.settle("a"); const second = s.settle("b");
    h.calls[0]!.resolve({}); await first; expect(s.contextFor("open")).toBeUndefined();
    h.calls[1]!.resolve({ cache_partition: "other" }); await second; expect(s.contextFor("open")).toBeUndefined();
    const third = s.settle("c"); h.calls[2]!.resolve({ row: "zzz" }); await third; expect(s.contextFor("open")).toBeUndefined();
  });
  it("flush sends a held selection before an activation and waits for its context; settling the same row twice sends once", async () => {
    const h = host(); const s = new Selection("notes", h);
    s.paint("a");
    const f = s.flush(); expect(h.calls.length).toBe(1); h.calls[0]!.resolve({}); await f;
    expect(s.contextFor("open")).toBe("c-a");
    await s.settle("a"); expect(h.calls.length).toBe(1);
  });
});
