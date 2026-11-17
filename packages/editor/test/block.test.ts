import { markdownLanguage } from "@codemirror/lang-markdown";
import { LanguageSupport, ensureSyntaxTree } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { describe, expect, test } from "vitest";
import { blockAt, blocksIn, buildDrawn, continueList, deleteBlock, duplicateBlock, headingsOf, indentItem, moveBlock, triggerAt } from "../src/block/index";

const state = (doc: string, anchor = 0) => { const s = EditorState.create({ doc, selection: { anchor }, extensions: [new LanguageSupport(markdownLanguage)] }); ensureSyntaxTree(s, doc.length, 5000); return s; };
const apply = (s: EditorState, ch: unknown) => s.update({ changes: ch as never }).state.doc.toString();

describe("blocks", () => {
  const doc = "# Title\n\nA paragraph.\n\n- one\n  - nested\n- two\n\n```ts\ncode\n```\n\n> quote\n";
  test("headings, paragraphs, items (nested ones on their own), code and quotes are blocks", () => {
    const kinds = blocksIn(state(doc)).map((b) => b.kind);
    expect(kinds).toEqual(["heading1", "paragraph", "item", "item", "item", "code", "quote"]);
  });
  test("a nested item has depth 1 and its parent's block stops before it", () => {
    const bs = blocksIn(state(doc));
    expect(bs[3]!.depth).toBe(1);
    expect(doc.slice(bs[2]!.from, bs[2]!.to)).toBe("- one");
  });
  test("the host can classify blocks", () => {
    const s = state("- [ ] a\n- b");
    const bs = blocksIn(s, 0, s.doc.length, (b, st) => (/^- \[ \]/.test(st.doc.sliceString(b.from, b.to)) ? "task" : null));
    expect(bs.map((b) => b.kind)).toEqual(["task", "item"]);
  });
  test("blockAt finds the innermost block", () => {
    const s = state(doc);
    expect(blockAt(s, doc.indexOf("nested"))?.depth).toBe(1);
  });
  test("delete, duplicate and move are single changes", () => {
    const s = state("one\n\ntwo\n\nthree");
    const [a, b] = blocksIn(s);
    expect(apply(s, duplicateBlock(s, a!))).toBe("one\none\n\ntwo\n\nthree");
    expect(apply(s, deleteBlock(s, b!))).toBe("one\n\n\nthree");
    const mv = moveBlock(s, a!, s.doc.toString().indexOf("three"))!;
    expect(s.update({ changes: mv as never }).state.doc.toString()).toContain("two");
    expect(moveBlock(s, a!, 0)).toBeNull();
  });
});

describe("headings", () => {
  test("level, text and position, for the contents rail", () => {
    const h = headingsOf(state("# A\n\ntext\n\n## B ##\n\n### C"));
    expect(h.map((x) => [x.level, x.text])).toEqual([[1, "A"], [2, "B"], [3, "C"]]);
  });
});

describe("lists", () => {
  const run = (doc: string, at: number, cmd: (t: any) => boolean) => { // eslint-disable-line @typescript-eslint/no-explicit-any
    let s = state(doc, at);
    const ok = cmd({ state: s, dispatch: (tr: { state: EditorState }) => { s = tr.state; } });
    return { ok, text: s.doc.toString() };
  };
  test("Enter continues a bullet, the next number and a fresh task", () => {
    expect(run("- a", 3, continueList).text).toBe("- a\n- ");
    expect(run("1. a", 4, continueList).text).toBe("1. a\n2. ");
    expect(run("- [x] a", 7, continueList).text).toBe("- [x] a\n- [ ] ");
  });
  test("Enter on an empty item ends the list", () => expect(run("- a\n- ", 6, continueList).text).toBe("- a\n"));
  test("Enter outside a list falls through", () => expect(run("plain", 5, continueList).ok).toBe(false));
  test("Tab indents and Shift-Tab outdents", () => {
    expect(run("- a", 3, indentItem(1)).text).toBe("  - a");
    expect(run("  - a", 5, indentItem(-1)).text).toBe("- a");
  });
});

describe("the drawn document", () => {
  const drawn = (doc: string) => { const d = buildDrawn(state(doc)); const hidden: string[] = []; d.atomic.between(0, doc.length, (f, t) => { hidden.push(doc.slice(f, t)); }); return hidden; };
  test("heading, emphasis and code marks are hidden", () => {
    expect(drawn("## Hi **bold** and `code`")).toEqual(["## ", "**", "**", "`", "`"]);
  });
  test("link syntax around the words is hidden", () => {
    expect(drawn("see [words](https://x.test) now")).toEqual(["[", "](https://x.test)"]);
  });
  test("list markers and task boxes are drawn as widgets", () => {
    expect(drawn("- a\n- [ ] b")).toEqual(["- ", "- [ ]"]); // the space after a box stays as the gap before the text
  });
  test("a quote mark and a rule are hidden or replaced", () => {
    expect(drawn("> quoted")).toEqual(["> "]);
    expect(drawn("a\n\n---\n\nb")).toEqual(["---"]);
  });
});

describe("triggers", () => {
  const at = (doc: string, pos = doc.length) => triggerAt(state(doc, pos));
  test("@, [[, # and a leading / at the caret", () => {
    expect(at("ask @pr")).toMatchObject({ kind: "mention", query: "pr", from: 4 });
    expect(at("see [[Stor")).toMatchObject({ kind: "link", query: "Stor", from: 4 });
    expect(at("a #tag")).toMatchObject({ kind: "tag", query: "tag" });
    expect(at("/rev")).toMatchObject({ kind: "skill", query: "rev", from: 0 });
  });
  test("not in code, not mid-word, not with a selection", () => {
    expect(at("`@pr`", 4)).toBeNull();
    expect(at("mail a@b")).toBeNull();
    expect(at("and/or")).toBeNull();
  });
});
