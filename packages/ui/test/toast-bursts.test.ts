import { describe, expect, test } from "vitest";
import { Bursts, burstKey, undoAll, type ToastEvent } from "../src/toast/bursts";

const ev = (over: Partial<ToastEvent> = {}): ToastEvent => ({
  op: "block.delete",
  context: "note-a",
  severity: "result",
  message: "Block deleted",
  undo: () => true,
  ...over,
});

describe("bursts", () => {
  test("twelve of the same event are one toast counting to 12", () => {
    const b = new Bursts(4000);
    let last;
    for (let i = 0; i < 12; i++) last = b.add(ev(), 1000 + i * 100);
    expect(last!.count).toBe(12);
    expect(last!.tokens).toHaveLength(12);
    expect(last!.event.message).toBe("Block deleted");
  });

  test("never grouped by words: a different note, severity, follow-up or operation stays apart", () => {
    const b = new Bursts();
    const a = b.add(ev(), 0);
    expect(b.add(ev({ context: "note-b" }), 10).id).not.toBe(a.id);
    expect(b.add(ev({ severity: "error" }), 20).id).not.toBe(a.id);
    expect(b.add(ev({ undo: undefined }), 30).id).not.toBe(a.id);
    expect(b.add(ev({ op: "block.archive", message: "Block deleted" }), 40).id).not.toBe(a.id);
    expect(b.add(ev(), 50).id).toBe(a.id);
  });

  test("the same words for different things never share a key", () => {
    expect(burstKey(ev({ context: "x" }))).not.toBe(burstKey(ev({ context: "y" })));
  });

  test("after a quiet gap the next event starts again at 1", () => {
    const b = new Bursts(4000);
    const first = b.add(ev(), 0);
    b.add(ev(), 1000);
    const later = b.add(ev(), 6000);
    expect(later.id).not.toBe(first.id);
    expect(later.count).toBe(1);
  });

  test("a dismissed or undone burst stops growing", () => {
    const b = new Bursts();
    const first = b.add(ev(), 0);
    b.close(first.id);
    expect(b.isOpen(first.id)).toBe(false);
    const next = b.add(ev(), 10);
    expect(next.id).not.toBe(first.id);
    expect(next.count).toBe(1);
  });

  test("waits never merge", () => {
    const b = new Bursts();
    const a = b.add(ev({ severity: "loading", undo: undefined }), 0);
    expect(b.add(ev({ severity: "loading", undo: undefined }), 10).id).not.toBe(a.id);
  });
});

describe("undo", () => {
  test("all undone: says all", async () => {
    const order: number[] = [];
    const out = await undoAll([1, 2, 3].map((i) => () => (order.push(i), true)));
    expect(out).toEqual({ undone: 3, total: 3, left: [] });
    expect(order).toEqual([3, 2, 1]); // newest first
  });

  test("a failure stops there and keeps the rest to try again", async () => {
    const t = [() => true, () => false, () => true];
    const out = await undoAll(t);
    expect(out.undone).toBe(1);
    expect(out.total).toBe(3);
    expect(out.left).toEqual(t.slice(0, 2));
  });

  test("a thrown error counts as not undone", async () => {
    const out = await undoAll([
      () => {
        throw new Error("gone");
      },
    ]);
    expect(out.undone).toBe(0);
    expect(out.left).toHaveLength(1);
  });

  test("async tokens are awaited in order", async () => {
    const order: number[] = [];
    const tok = (i: number, ms: number) => () => new Promise<boolean>((ok) => setTimeout(() => (order.push(i), ok(true)), ms));
    await undoAll([tok(1, 1), tok(2, 20)]);
    expect(order).toEqual([2, 1]);
  });
});
