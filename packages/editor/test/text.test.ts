import { copyLineDown, copyLineUp, indentLess, indentMore, moveLineDown, moveLineUp, selectLine as cmSelectLine } from "@codemirror/commands";
import { EditorSelection, EditorState } from "@codemirror/state";
import { describe, expect, test } from "vitest";
import { apply } from "../src/transaction";
import { COMMANDS, FieldEditor, KEYMAP, matchKey, matchTrigger, pickerKey, stringDoc, textAttrs } from "../src/index";

const DOC = "one\n  two\nthree\n\nfive";
const SELS: [number, number][] = [[0, 0], [5, 5], [4, 12], [14, 14], [3, 8], [19, 19], [6, 6], [8, 17]];

/** Runs a shared command and CodeMirror's own command on the same text and selection and returns both results. */
function both(name: keyof typeof COMMANDS, cm: (t: { state: EditorState; dispatch: (tr: any) => void }) => boolean, [a, h]: [number, number]) {
  const ours = COMMANDS[name](stringDoc(DOC), { anchor: a, head: h });
  let state = EditorState.create({ doc: DOC, selection: EditorSelection.single(a, h), extensions: [EditorState.allowMultipleSelections.of(false)] });
  let theirs: EditorState | null = null;
  cm({ state, dispatch: (tr) => { theirs = tr.state; } });
  return { ours, theirs: theirs as EditorState | null };
}

describe("shared commands agree with CodeMirror's (the behaviour the block editor had)", () => {
  const cases: [keyof typeof COMMANDS, any][] = [["moveLineUp", moveLineUp], ["moveLineDown", moveLineDown], ["copyLineUp", copyLineUp], ["copyLineDown", copyLineDown], ["indentMore", indentMore], ["indentLess", indentLess]];
  for (const [name, cm] of cases) {
    test(name, () => {
      for (const sel of SELS) {
        const { ours, theirs } = both(name, cm, sel);
        if (!theirs) { expect(ours === null || name === "deleteLine").toBe(true); continue; }
        if (!ours) { expect(theirs.doc.toString()).toBe(DOC); continue; }
        expect(apply(DOC, ours.change), `${name} ${sel}`).toBe(theirs.doc.toString());
        const m = theirs.selection.main;
        if (name !== "indentMore" && name !== "indentLess" && !(name === "copyLineUp" && sel[0] !== sel[1])) expect([ours.selection.anchor, ours.selection.head], `${name} selection ${sel}`).toEqual([m.anchor, m.head]);
      }
    });
  }
  test("deleteLine removes the lines and one line break and puts the caret where the text closes up", () => {
    const d = stringDoc(DOC);
    const run = (a: number) => { const e = COMMANDS.deleteLine(d, { anchor: a, head: a }); return [apply(DOC, e.change), e.selection.head] as const; };
    expect(run(6)).toEqual(["one\nthree\n\nfive", 4]);
    expect(run(19)).toEqual(["one\n  two\nthree\n", 16]);
    expect(run(0)).toEqual(["  two\nthree\n\nfive", 0]);
  });
  test("selectLine selects the line with its break", () => {
    const e = COMMANDS.selectLine(stringDoc(DOC), { anchor: 6, head: 6 })!;
    expect(e.selection).toEqual({ anchor: 4, head: 10 });
    let theirs: EditorState | null = null;
    cmSelectLine({ state: EditorState.create({ doc: DOC, selection: EditorSelection.single(6) }), dispatch: (tr) => { theirs = tr.state; } });
    expect([theirs!.selection.main.from, theirs!.selection.main.to]).toEqual([4, 10]);
  });
  test("Escape collapses a selection to the caret and does nothing without one", () => {
    expect(COMMANDS.collapse(stringDoc("abc"), { anchor: 0, head: 2 })!.selection).toEqual({ anchor: 2, head: 2 });
    expect(COMMANDS.collapse(stringDoc("abc"), { anchor: 1, head: 1 })).toBeNull();
  });
});

describe("keys, triggers, attributes", () => {
  test("every shared shortcut matches its own keydown", () => {
    const ev = (k: Partial<KeyboardEvent>) => k as KeyboardEvent;
    expect(matchKey(ev({ key: "ArrowUp", altKey: true }), true)).toBe("moveLineUp");
    expect(matchKey(ev({ key: "ArrowUp", altKey: true, shiftKey: true }), true)).toBe("copyLineUp");
    expect(matchKey(ev({ key: "ArrowUp", altKey: true }), false)).toBeNull(); // a one-line field has no lines to move
    expect(matchKey(ev({ key: "Escape" }), false)).toBe("collapse");
    expect(KEYMAP.length).toBeGreaterThan(5);
  });
  test("one set of trigger rules", () => {
    expect(matchTrigger("see [[Stor")).toEqual({ kind: "link", query: "Stor", lead: 2 });
    expect(matchTrigger("hi @Pri")).toEqual({ kind: "mention", query: "Pri", lead: 1 });
    expect(matchTrigger("a #tag")).toEqual({ kind: "tag", query: "tag", lead: 1 });
    expect(matchTrigger("/rev")).toEqual({ kind: "skill", query: "rev", lead: 1 });
    expect(matchTrigger("mail me@x.org")).toBeNull();
    expect(matchTrigger("hi @Pri", ["skill"])).toBeNull();
  });
  test("picker keys: wrap, choose, dismiss", () => {
    const log: string[] = [];
    const k = (key: string) => pickerKey({ key, preventDefault() {} }, { count: 3, index: 0, set: (i) => log.push(`set${i}`), choose: () => log.push("choose"), dismiss: () => log.push("dismiss") });
    expect([k("ArrowUp"), k("ArrowDown"), k("Enter"), k("Tab"), k("Escape"), k("a")]).toEqual([true, true, true, true, true, false]);
    expect(log).toEqual(["set2", "set1", "choose", "choose", "dismiss"]);
  });
  test("prose gets spelling and capitals, a search field does not", () => {
    expect(textAttrs("prose").spellcheck).toBe("true");
    expect(textAttrs("search").spellcheck).toBe("false");
  });
});

describe("a text field behaves as the editor does", () => {
  const mk = (tag: "textarea" | "input", text: string, o = {}) => { const el = document.createElement(tag); document.body.append(el); el.value = text; return { el, fe: new FieldEditor(el, o) }; };
  const key = (el: HTMLElement, init: KeyboardEventInit) => { const e = new KeyboardEvent("keydown", { bubbles: true, cancelable: true, ...init }); el.dispatchEvent(e); return e; };
  test("Alt-Up moves the line, as one undo step, and the selection goes with it", () => {
    const { el, fe } = mk("textarea", "one\ntwo\nthree");
    el.setSelectionRange(5, 5);
    const e = key(el, { key: "ArrowUp", altKey: true });
    expect(e.defaultPrevented).toBe(true);
    expect(el.value).toBe("two\none\nthree");
    expect(el.selectionStart).toBe(1);
    fe.undo();
    expect(el.value).toBe("one\ntwo\nthree");
  });
  test("a one-line field leaves Alt-Up alone", () => {
    const { el } = mk("input", "abc");
    expect(key(el, { key: "ArrowUp", altKey: true }).defaultPrevented).toBe(false);
  });
  test("Escape collapses a selection when the host does not use it to cancel", () => {
    const { el } = mk("textarea", "abc", { escapeCollapses: true });
    el.setSelectionRange(0, 3);
    key(el, { key: "Escape" });
    expect([el.selectionStart, el.selectionEnd]).toEqual([3, 3]);
    const b = mk("textarea", "abc", { escapeCollapses: false });
    b.el.setSelectionRange(0, 3);
    expect(key(b.el, { key: "Escape" }).defaultPrevented).toBe(false);
  });
  test("a paste into a one-line field turns line breaks into spaces, as a step of its own", () => {
    const { el, fe } = mk("input", "ab");
    el.setSelectionRange(2, 2);
    const ev = new Event("paste", { bubbles: true, cancelable: true }) as any;
    ev.clipboardData = { getData: () => "x\ny\r\nz" };
    el.dispatchEvent(ev);
    expect(el.value).toBe("abx y z");
    fe.undo();
    expect(el.value).toBe("ab");
  });
  test("the key that confirms a composition is not an Enter", () => {
    const { fe } = mk("textarea", "");
    expect(fe.composing({ key: "Enter", isComposing: false, keyCode: 229 } as KeyboardEvent)).toBe(true);
    expect(fe.composing({ key: "Enter", isComposing: false, keyCode: 13 } as KeyboardEvent)).toBe(false);
    fe.ime.begin(); fe.ime.end();
    expect(fe.composing({ key: "Enter", isComposing: false, keyCode: 13 } as KeyboardEvent)).toBe(true); // within a few ms of compositionend
  });
  test("a field reports the same picker triggers the editor does", () => {
    const seen: (string | null)[] = [];
    const { el } = mk("textarea", "hello @Pri", { triggers: { kinds: ["mention", "skill"], on: (t: any) => seen.push(t ? `${t.kind}:${t.query}:${t.from}` : null) } });
    el.setSelectionRange(10, 10);
    el.dispatchEvent(new Event("input", { bubbles: true }));
    expect(seen).toEqual(["mention:Pri:6"]);
  });
  test("spelling is on for prose fields", () => {
    const { el } = mk("textarea", "");
    expect(el.getAttribute("spellcheck")).toBe("true");
  });
});
