import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { ensureSyntaxTree } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { describe, expect, test } from "vitest";
import { alignsOf, cellsOf } from "../src/block/tables";
import { frontmatterEnd } from "../src/block/frontmatter";
import { continueList } from "../src/block/lists";
import { detect } from "../src/block/features";
import { endMarker, ensureId, idsIn, isTaskLine, markersIn, newId, visibleText } from "../src/block/ids";

const st = (doc: string, anchor = 0) => { const s = EditorState.create({ doc, selection: { anchor }, extensions: [markdown({ base: markdownLanguage })] }); ensureSyntaxTree(s, doc.length, 5000); return s; };

describe("block ID markers (ADR 0056)", () => {
  test("a marker is ^ and 3+ characters after a space, bracket or comma, before a space or the end", () => {
    expect(markersIn("do it ^k2m8q1w4zre7k3f9").map((m) => m.id)).toEqual(["k2m8q1w4zre7k3f9"]);
    expect(markersIn("see [x](#^abc) and a^b and 2^10 and ^a")).toEqual([]);
    expect(markersIn("a ^abc, b ^def").map((m) => m.id)).toEqual(["abc", "def"]);
  });
  test("a marker at the end of a line takes its one separating space; one inside a line does not", () => {
    expect(markersIn("text   ^abc1")[0]).toEqual({ from: 6, to: 12, id: "abc1" });
    expect(markersIn("a ^abc b")[0]).toEqual({ from: 2, to: 6, id: "abc" });
  });
  test("visibleText removes every marker and nothing else", () => {
    expect(visibleText("- [ ] task ^abc123\nplain ^k2m8q1w4zre7k3f9\nstays ^a")).toBe("- [ ] task\nplain\nstays ^a");
  });
  test("new IDs are 16 Crockford characters and never collide with a taken one", () => {
    const id = newId(new Set());
    expect(id).toMatch(/^[0-9a-hjkmnp-tv-z]{16}$/);
    let n = 0;
    const rand = (k: number) => new Uint8Array(k).fill(n++ === 0 ? 0 : 1);
    expect(newId(new Set(["0000000000000000"]), rand)).toBe("1111111111111111");
  });
  test("ensureId adds a marker to a block with none, and returns the existing ID otherwise", () => {
    const s = st("- [ ] one\n- [ ] two ^abc123");
    const a = ensureId(s, 0);
    expect(a.change?.insert).toMatch(/^ \^[0-9a-z]{16}$/);
    expect(a.change?.from).toBe(9);
    expect(ensureId(s, 12)).toEqual({ id: "abc123", change: null });
    expect(idsIn(s)).toEqual(["abc123"]);
  });
  test("task lines and end markers", () => {
    expect(isTaskLine("  - [x] done")).toBe(true);
    expect(isTaskLine("- plain")).toBe(false);
    expect(endMarker("a ^abc")?.id).toBe("abc");
    expect(endMarker("a ^abc b")).toBeNull();
  });
  test("Enter in an item with a marker keeps the marker on the first part and moves the rest", () => {
    const doc = "- [ ] hello world ^abc123";
    let out = "";
    const s = st(doc, 12);
    continueList({ state: s, dispatch: (tr) => { out = tr.state.doc.toString(); } });
    expect(out).toBe("- [ ] hello  ^abc123\n- [ ] world");
  });
  test("an item holding only a marker counts as empty: Enter ends the list", () => {
    let out = "x";
    continueList({ state: st("- ^abc123", 2), dispatch: (tr) => { out = tr.state.doc.toString(); } });
    expect(out).toBe("");
  });
});

describe("frontmatter", () => {
  test("ends after the closing line break, or is absent", () => {
    expect(frontmatterEnd(st("---\ntitle: x\n---\nBody"))).toBe(17);
    expect(frontmatterEnd(st("---\ntitle: x\n---"))).toBe(0); // no line break after it: nowhere to type
    expect(frontmatterEnd(st("Body\n---\nx\n---\n"))).toBe(0);
  });
});

describe("tables", () => {
  test("cells split on unescaped bars; alignment reads the delimiter row", () => {
    expect(cellsOf("| a | b \\| c |")).toEqual(["a", "b | c"]);
    expect(alignsOf("|:--|:-:|--:|---|")).toEqual(["left", "center", "right", null]);
  });
});

describe("what a note needs", () => {
  test("detect names the pieces by cheap patterns", () => {
    expect(detect("plain text\n\nmore")).toEqual([]);
    expect(detect("---\na: b\n---\nx")).toContain("frontmatter");
    expect(detect("- [ ] task")).toContain("ids");
    expect(detect("x ^abc123")).toContain("ids");
    expect(detect("| a | b |\n| --- | --- |\n| 1 | 2 |")).toContain("tables");
    expect(detect("```ts\nlet a\n```")).toContain("code");
    expect(detect("```\nplain\n```")).not.toContain("code");
  });
});
