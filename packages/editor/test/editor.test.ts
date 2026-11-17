import { describe, expect, test } from "vitest";
import { apply, diff, FieldEditor, History, ImeGate, invert, mapPos, snap } from "../src/index";

describe("changes", () => {
  test("apply, invert and diff round-trip", () => {
    const a = "hello world", b = "hello brave new world";
    const c = diff(a, b)!;
    expect(apply(a, c)).toBe(b);
    expect(apply(b, invert(a, c))).toBe(a);
    expect(diff("same", "same")).toBeNull();
  });
  test("positions count UTF-16 and never split a surrogate pair", () => {
    const t = "a😀b";
    expect(t.length).toBe(4);
    expect(snap(t, 2)).toBe(1);
    expect(snap(t, 3)).toBe(3);
    const c = diff("a😀b", "a😁b")!;
    expect(c.from).toBe(1);
    expect(apply("a😀b", c)).toBe("a😁b");
  });
  test("a range outside the text is refused", () => expect(() => apply("abc", { from: 2, to: 9, insert: "" })).toThrow(RangeError));
  test("mapPos shifts after a change and pulls a position inside it to its start", () => {
    const c = { from: 2, to: 4, insert: "xyz" };
    expect([mapPos(1, c), mapPos(3, c), mapPos(6, c)]).toEqual([1, 2, 7]);
  });
});

describe("history", () => {
  let now = 0;
  const h = () => new History({ now: () => now, idleMs: 600 });
  test("typing coalesces into one step, a pause ends it", () => {
    now = 0; const hist = h();
    let text = "";
    for (const ch of "abc") { const c = { from: text.length, to: text.length, insert: ch }; hist.record(text, c, "user"); text = apply(text, c); now += 100; }
    now += 1000;
    const c = { from: 3, to: 3, insert: "d" }; hist.record(text, c, "user"); text = apply(text, c);
    for (const u of hist.undo(text)!) text = apply(text, u);
    expect(text).toBe("abc");
    for (const u of hist.undo(text)!) text = apply(text, u);
    expect(text).toBe("");
    expect(hist.canUndo).toBe(false);
  });
  test("redo brings it back, and a new change clears redo", () => {
    now = 0; const hist = h();
    let text = "";
    const ins = (s: string) => { const c = { from: text.length, to: text.length, insert: s }; hist.record(text, c, "user"); text = apply(text, c); };
    ins("hi"); for (const u of hist.undo(text)!) text = apply(text, u);
    expect(text).toBe("");
    for (const r of hist.redo(text)!) text = apply(text, r);
    expect(text).toBe("hi");
    for (const u of hist.undo(text)!) text = apply(text, u);
    ins("x");
    expect(hist.canRedo).toBe(false);
  });
  test("a command's change is its own step", () => {
    now = 0; const hist = h();
    let text = "ask ";
    const c = { from: 4, to: 4, insert: "@Priya " }; hist.record(text, c, "command"); text = apply(text, c);
    for (const u of hist.undo(text)!) text = apply(text, u);
    expect(text).toBe("ask ");
  });
  test("foreign changes are not undone and shift what is recorded", () => {
    now = 0; const hist = h();
    let text = "bc";
    const mine = { from: 2, to: 2, insert: "d" }; hist.record(text, mine, "user"); text = apply(text, mine); // "bcd"
    const theirs = { from: 0, to: 0, insert: "aa" }; hist.record(text, theirs, "remote"); text = apply(text, theirs); // "aabcd"
    for (const u of hist.undo(text)!) text = apply(text, u);
    expect(text).toBe("aabc");
  });
  test("bounded", () => {
    now = 0; const hist = new History({ now: () => now, limit: 3, idleMs: 0 });
    let text = "";
    for (let i = 0; i < 10; i++) { now += 10; const c = { from: text.length, to: text.length, insert: "x" }; hist.record(text, c, "command"); text = apply(text, c); }
    let n = 0; while (hist.undo(text)) { n++; text = text.slice(0, -1); }
    expect(n).toBe(3);
  });
});

describe("the input-method gate", () => {
  test("work waits while composing and runs in order after", () => {
    const g = new ImeGate(); const log: number[] = [];
    g.hold(() => log.push(1));
    g.begin(); g.hold(() => log.push(2)); g.hold(() => log.push(3));
    expect(log).toEqual([1]);
    g.end();
    expect(log).toEqual([1, 2, 3]);
  });
});

describe("the field editor", () => {
  const make = (value = "") => { const el = document.createElement("textarea"); el.value = value; document.body.append(el); const txs: unknown[] = []; const f = new FieldEditor(el, { onchange: (t) => txs.push(t), idleMs: 600 }); return { el, f, txs }; };
  const type = (el: HTMLTextAreaElement, s: string) => { el.value += s; el.dispatchEvent(new Event("input", { bubbles: true })); };
  test("typing is a user change; undo and redo go through the history", () => {
    const { el, f, txs } = make();
    type(el, "h"); type(el, "i");
    expect(txs).toHaveLength(2);
    expect((txs[0] as { origin: string }).origin).toBe("user");
    expect(f.undo()).toBe(true);
    expect(el.value).toBe("");
    expect(f.redo()).toBe(true);
    expect(el.value).toBe("hi");
  });
  test("a command (a picked mention) is one undo step and leaves the caret after it", () => {
    const { el, f } = make("ask ");
    f.apply({ from: 4, to: 4, insert: "@Priya " });
    expect(el.value).toBe("ask @Priya ");
    expect(el.selectionStart).toBe(11);
    f.undo();
    expect(el.value).toBe("ask ");
  });
  test("a remote change waits while composing and then applies", () => {
    const { el, f } = make("abc");
    el.dispatchEvent(new Event("compositionstart"));
    expect(f.apply({ from: 0, to: 0, insert: "X" }, "remote")).toBe(false);
    expect(el.value).toBe("abc");
    el.dispatchEvent(new Event("compositionend"));
    expect(el.value).toBe("Xabc");
  });
  test("a remote change keeps the caret where the text it follows is", () => {
    const { el, f } = make("hello world");
    el.setSelectionRange(11, 11);
    f.apply({ from: 0, to: 0, insert: ">> " }, "remote");
    expect(el.selectionStart).toBe(14);
  });
  test("reset replaces the text and forgets the history", () => {
    const { el, f } = make();
    type(el, "x"); f.reset("draft");
    expect(f.undo()).toBe(false);
    expect(el.value).toBe("draft");
  });
  test("Ctrl-Z is handled by the one history", () => {
    const { el, f } = make();
    type(el, "x");
    const e = new KeyboardEvent("keydown", { key: "z", ctrlKey: true, bubbles: true, cancelable: true });
    el.dispatchEvent(e);
    expect(e.defaultPrevented).toBe(true);
    expect(el.value).toBe("");
    f.destroy();
  });
});
