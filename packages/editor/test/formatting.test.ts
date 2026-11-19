import { markdownLanguage } from "@codemirror/lang-markdown";
import { LanguageSupport, ensureSyntaxTree } from "@codemirror/language";
import { EditorSelection, EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { describe, expect, test } from "vitest";
import { deleteBlocks, duplicateBlocks, marksAround, nudgeBlocks, setType, shortcuts, spansIn, toggleMark, typeOfLine, withChildren, type Shortcuts } from "../src/block/index";

// The bar's edits (src/block/formatting.ts) are plain changes of the Markdown text: each test names the text before and after.
function make(doc: string, from = 0, to = from) {
  const view = new EditorView({ state: EditorState.create({ doc, selection: EditorSelection.range(from, to), extensions: [new LanguageSupport(markdownLanguage)] }), parent: document.body });
  ensureSyntaxTree(view.state, doc.length, 5000);
  view.focus = () => {}; // (a command gives the editor its focus back; a document with no focus has nothing to restore)
  return view;
}
const text = (v: EditorView) => v.state.doc.toString();

describe("marks", () => {
  test("wrap the selected words and unwrap them again, keeping the words selected", () => {
    const v = make("one two three", 4, 7);
    toggleMark(v, "bold");
    expect(text(v)).toBe("one **two** three");
    expect(v.state.sliceDoc(v.state.selection.main.from, v.state.selection.main.to)).toBe("two");
    toggleMark(v, "bold");
    expect(text(v)).toBe("one two three");
  });
  test("each mark writes its own characters", () => {
    for (const [kind, m] of [["italic", "*"], ["strike", "~~"], ["code", "`"], ["highlight", "=="]] as const) {
      const v = make("a b c", 2, 3);
      toggleMark(v, kind);
      expect(text(v)).toBe(`a ${m}b${m} c`);
    }
  });
  test("with only a caret the mark starts here, and an empty pair is taken back", () => {
    const v = make("ab", 1);
    toggleMark(v, "bold");
    expect(text(v)).toBe("a****b");
    expect(v.state.selection.main.head).toBe(3);
    toggleMark(v, "bold");
    expect(text(v)).toBe("ab");
  });
  test("which marks surround the words (italic is not bold's star)", () => {
    const s = make("x **b** y *i* z", 4, 5).state;
    expect(marksAround(s, 4, 5)).toEqual({ bold: true, italic: false, strike: false, code: false, highlight: false });
    expect(marksAround(s, 11, 12).italic).toBe(true);
  });
  test("a mark is one undoable edit of the text, with no other representation", () => {
    const v = make("one two", 0, 3);
    toggleMark(v, "code");
    expect(text(v)).toBe("`one` two");
  });
});

describe("block kinds", () => {
  test("a line's kind", () => {
    expect(["# a", "## a", "### a", "> a", "```", "---", "- a", "1. a", "- [ ] a", "a"].map(typeOfLine)).toEqual(["h1", "h2", "h3", "quote", "code", "divider", "li", "ol", "todo", "p"]);
  });
  test("turn into: a heading, a list with its indent kept, back to body", () => {
    const v = make("hello\n\n  - item");
    setType(v, [], "h2");
    expect(text(v)).toBe("## hello\n\n  - item");
    setType(v, [{ from: 10, to: 17 }], "todo");
    expect(text(v)).toBe("## hello\n\n  - [ ] item");
    setType(v, [{ from: 0, to: 8 }], "p");
    expect(text(v)).toBe("hello\n\n  - [ ] item");
  });
});

describe("whole-block actions", () => {
  const doc = "# Title\n\nFirst.\n\nSecond.\n\n- a\n- b\n";
  test("the blocks a range touches, below the frontmatter", () => {
    const v = make(doc);
    expect(spansIn(v.state, 0, 10).map((s) => doc.slice(s.from, s.to))).toEqual(["# Title", "First."]);
  });
  test("duplicate copies the block under itself with a blank line (a list item onto the next line)", () => {
    let v = make(doc, 10);
    expect(duplicateBlocks(v, [])).toBe(true);
    expect(text(v)).toBe("# Title\n\nFirst.\n\nFirst.\n\nSecond.\n\n- a\n- b\n");
    v = make(doc, doc.indexOf("- a") + 3);
    duplicateBlocks(v, []);
    expect(text(v)).toBe("# Title\n\nFirst.\n\nSecond.\n\n- a\n- a\n- b\n");
  });
  test("move down and up swap neighbouring blocks and report where the selection went", () => {
    const v = make(doc, 10);
    const moved = nudgeBlocks(v, [], 1);
    expect(text(v)).toBe("# Title\n\nSecond.\n\nFirst.\n\n- a\n- b\n");
    expect(v.state.sliceDoc(moved!.from, moved!.to)).toBe("First.");
    nudgeBlocks(v, [moved!], -1);
    expect(text(v)).toBe(doc);
    expect(nudgeBlocks(make(doc, 2), [], -1)).toBeNull();
  });
  test("delete takes the selected blocks with their line break", () => {
    const v = make(doc);
    expect(deleteBlocks(v, [{ from: 9, to: 15 }, { from: 17, to: 24 }])).toBe(2);
    expect(text(v)).toBe("# Title\n\n\n\n- a\n- b\n"); // (the blank lines around them stay: the text between blocks is not theirs)
    expect(deleteBlocks(v, [])).toBe(0);
  });
  test("a heading takes its section with it", () => {
    const v = make("# A\n\ntext\n\n## B\n\nmore\n\n# C\n");
    expect(withChildren(v.state, { from: 0, to: 3 }).to).toBe(v.state.doc.toString().indexOf("# C") - 1) // up to the blank line before the next heading;
  });
});

describe("the shortcuts", () => {
  test("a mark key acts only with words selected; Mod-K links with words and is the palette's with none", () => {
    const calls: string[] = [];
    const s: Shortcuts = {
      mark: (k) => calls.push(k), heading: (n) => calls.push(`h${n}`), move: (d) => calls.push(`move${d}`), selectBlock: () => calls.push("select"), blocksSelected: () => false, leave: () => calls.push("leave"),
      insert: () => calls.push("insert"), link: () => calls.push("link"), duplicate: () => (calls.push("dup"), true), remove: () => (calls.push("del"), true), copyLink: () => calls.push("copylink"),
    };
    const v = new EditorView({ state: EditorState.create({ doc: "words here", selection: EditorSelection.range(0, 5), extensions: [shortcuts(s, false)] }), parent: document.body });
    const press = (init: KeyboardEventInit) => { const e = new KeyboardEvent("keydown", { bubbles: true, cancelable: true, ...init }); v.contentDOM.dispatchEvent(e); return e.defaultPrevented; };
    expect(press({ key: "b", ctrlKey: true })).toBe(true);
    expect(press({ key: "i", ctrlKey: true })).toBe(true);
    expect(press({ key: "X", keyCode: 88, ctrlKey: true, shiftKey: true })).toBe(true);
    expect(press({ key: "e", ctrlKey: true })).toBe(true);
    expect(press({ key: "y", code: "KeyY", ctrlKey: true, shiftKey: true })).toBe(true);
    expect(press({ key: "k", ctrlKey: true })).toBe(true);
    expect(press({ key: "1", ctrlKey: true, altKey: true })).toBe(true);
    expect(press({ key: "ArrowDown", altKey: true })).toBe(true);
    expect(press({ key: "d", ctrlKey: true })).toBe(true);
    expect(press({ key: "K", keyCode: 75, ctrlKey: true, shiftKey: true })).toBe(true);
    expect(calls).toEqual(["bold", "italic", "strike", "code", "highlight", "link", "h1", "move1", "dup", "del"]);
    v.dispatch({ selection: { anchor: 3 } });
    calls.length = 0;
    expect(press({ key: "b", ctrlKey: true })).toBe(false); // no words selected: the browser keeps the key
    expect(press({ key: "k", ctrlKey: true })).toBe(false);
    expect(press({ key: "Escape" })).toBe(true);
    expect(calls).toEqual(["select"]);
  });
});
