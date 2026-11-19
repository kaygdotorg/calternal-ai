import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { describe, expect, it, vi } from "vitest";
import { THEMES, hasSurface, pageVars, setThemeVars } from "./apply";
import { caretFacts, convertChanges, toggleBold, typeOfLine } from "./blocks";
import { blockEnd, pairs, settingsWatch } from "./frontmatter";
import { patchFrontmatter } from "./keys";
import { pageStyle } from "./page";
import { noteStyle } from "./store.svelte";
import { caretWatch } from "./watch";

const NOTE = '---\ntitle: Look\ncalternal-theme: nord\ncalternal-text-size: "large"\n---\n# Look\n\nOne thing at a time.\n';
const at = (doc: string, anchor: number, head = anchor) => EditorState.create({ doc, selection: { anchor, head } });
const apply = (text: string, patch: Record<string, string | null>) => { const c = patchFrontmatter(text, patch); return c ? text.slice(0, c.from) + c.insert + text.slice(c.to) : text; };

describe("style keys are plain frontmatter lines, written in one change", () => {
  it("a theme is one line; taking it away leaves the note as it was, byte for byte", () => {
    const plain = "# Look\n\nOne thing at a time.\n";
    const on = apply(plain, { "calternal-theme": "nord" });
    expect(on).toBe("---\ncalternal-theme: nord\n---\n# Look\n\nOne thing at a time.\n");
    // the empty block the removal leaves is kept as the writer left it (the existing app's rule); the body is untouched
    expect(apply(on, { "calternal-theme": null })).toBe("---\n---\n" + plain);
  });
  it("several keys change in one edit, other lines stay exactly as they were", () => {
    const out = apply(NOTE, { "calternal-theme": null, "calternal-page-opacity": "80%", "calternal-page-blur": "20px" });
    expect(out).toBe('---\ntitle: Look\ncalternal-text-size: "large"\ncalternal-page-opacity: 80%\ncalternal-page-blur: 20px\n---\n# Look\n\nOne thing at a time.\n');
    expect(patchFrontmatter(NOTE, { "calternal-theme": "nord" })).toBeNull();
  });
});

describe("the page style", () => {
  it("reads the keys; an unknown value is the default and a phone is always full width", () => {
    const p = pageStyle({ "calternal-width": "wide", "calternal-font": "serif", "calternal-text-size": "large", "calternal-page-opacity": "80%", "calternal-page-blur": "20px", "calternal-frame": "off", "calternal-grain": "on", "calternal-page-colour": "sand", "calternal-theme": "nord", "calternal-lines": "dotted" });
    expect(p).toMatchObject({ width: "wide", font: "system", size: "large", opacity: 0.8, blur: 20, frame: false, grain: true, colour: "sand", theme: "nord", lines: "dotted" });
    expect(pageStyle({ "calternal-width": "wide" }, true).width).toBe("full");
    expect(pageStyle({}).width).toBe("narrow");
    expect(pageStyle({ "calternal-page-colour": "nope", "calternal-theme": "No Way" })).toMatchObject({ colour: "default", theme: "" });
  });
  it("a plain note has no surface and no variables; any page key makes one", () => {
    expect(hasSurface(pageStyle({}), {})).toBe(false);
    expect(pageVars(pageStyle({}))).toBe("");
    expect(hasSurface(pageStyle({ "calternal-theme": "nord" }), { "calternal-theme": "nord" })).toBe(true);
    expect(pageVars(pageStyle({ "calternal-page-opacity": "80%", "calternal-page-blur": "20px", "calternal-page-colour": "sand" }))).toContain("--page-opacity:0.8");
  });
  it("a theme sets the palette tokens on its card only and clears them again", () => {
    const el = document.createElement("div");
    expect(THEMES.length).toBeGreaterThan(3);
    setThemeVars(el, "nord", "dark");
    expect(el.style.getPropertyValue("--paper")).toBe("#2e3440");
    setThemeVars(el, "", "dark");
    expect(el.style.getPropertyValue("--paper")).toBe("");
    expect(document.documentElement.style.getPropertyValue("--paper")).toBe("");
  });
});

describe("reading the frontmatter in the editor", () => {
  it("pairs come from the block only, unquoted", () => {
    expect(pairs(at(NOTE, 0))).toEqual({ title: "Look", "calternal-theme": "nord", "calternal-text-size": "large" });
    expect(pairs(at("# a: b\n", 0))).toEqual({});
    expect(blockEnd(at(NOTE, 0).doc)).toBe(NOTE.indexOf("# Look"));
  });
  it("the host hears once at build and then only when a change touches the block, never for the body", () => {
    const seen = vi.fn();
    const view = new EditorView({ state: EditorState.create({ doc: NOTE, extensions: [settingsWatch(seen)] }), parent: document.body });
    expect(seen).toHaveBeenCalledTimes(1);
    const body = NOTE.indexOf("One thing");
    for (let i = 0; i < 30; i++) view.dispatch({ changes: { from: body, insert: "x" } });
    expect(seen).toHaveBeenCalledTimes(1);
    const at4 = view.state.doc.toString().indexOf("nord");
    view.dispatch({ changes: { from: at4, to: at4 + 4, insert: "dracula" } });
    expect(seen).toHaveBeenCalledTimes(2);
    expect(seen.mock.calls[1]![0]["calternal-theme"]).toBe("dracula");
    view.destroy();
  });
  it("a block created at the top of a plain note is noticed, and removing it is too", () => {
    const seen = vi.fn();
    const view = new EditorView({ state: EditorState.create({ doc: "# Plain\n", extensions: [settingsWatch(seen)] }), parent: document.body });
    view.dispatch({ changes: { from: 0, insert: "---\ncalternal-font: mono\n---\n" } });
    expect(seen).toHaveBeenLastCalledWith({ "calternal-font": "mono" });
    view.dispatch({ changes: { from: 0, to: "---\ncalternal-font: mono\n---\n".length } });
    expect(seen).toHaveBeenLastCalledWith({});
    view.destroy();
  });
});

describe("block kinds as text", () => {
  it("a line's kind", () => {
    expect(["# a", "## a", "### a", "> a", "```ts", "---", "- a", "1. a", "- [ ] a", "  - [x] a", "plain"].map(typeOfLine)).toEqual(["h1", "h2", "h3", "quote", "code", "divider", "li", "ol", "todo", "todo", "p"]);
  });
  const run = (doc: string, anchor: number, type: Parameters<typeof convertChanges>[1], head = anchor) => { const s = at(doc, anchor, head); return s.update({ changes: convertChanges(s, type) }).state.doc.toString(); };
  it("converts a paragraph, a heading, a nested item and several lines; code wraps in a fence; the frontmatter is untouched", () => {
    expect(run("Hello\n", 2, "h2")).toBe("## Hello\n");
    expect(run("Hello\n", 2, "todo")).toBe("- [ ] Hello\n");
    expect(run("## Hello\n", 4, "quote")).toBe("> Hello\n");
    expect(run("## Hello\n", 4, "p")).toBe("Hello\n");
    expect(run("- a\n  - b\n", 6, "ol")).toBe("- a\n  1. b\n");
    expect(run("a\nb\n", 0, "li", 3)).toBe("- a\n- b\n");
    expect(run("- a\n- b\n", 0, "code", 7)).toBe("```\na\nb\n```\n");
    const doc = "---\ncalternal-font: mono\n---\nBody\n";
    expect(run(doc, 1, "h1")).toBe(doc.replace("Body", "# Body"));
  });
  it("bold wraps, unwraps and opens an empty pair on a bare caret", () => {
    const s = at("some words", 5, 10), on = s.update(toggleBold(s)).state;
    expect(on.doc.toString()).toBe("some **words**");
    expect(caretFacts(on).bold).toBe(true);
    expect(on.update(toggleBold(on)).state.doc.toString()).toBe("some words");
    const bare = at("ab", 1), pair = bare.update(toggleBold(bare)).state;
    expect(pair.doc.toString()).toBe("a****b");
    expect(pair.update(toggleBold(pair)).state.doc.toString()).toBe("ab");
  });
});

describe("nothing runs per key unless the block tab watches", () => {
  it("the listener is installed on the first watch, silent when nobody watches, installed again after the state is replaced", () => {
    const view = new EditorView({ state: EditorState.create({ doc: "# T\n\nbody\n" }), parent: document.body });
    const seen = vi.fn();
    const stop = caretWatch(view, seen);
    expect(seen).toHaveBeenCalledTimes(1);
    view.dispatch({ selection: { anchor: 5 } });
    expect(seen).toHaveBeenCalledTimes(2);
    stop();
    view.dispatch({ selection: { anchor: 6 } });
    view.dispatch({ changes: { from: 0, insert: "y" } });
    expect(seen).toHaveBeenCalledTimes(2);
    view.setState(EditorState.create({ doc: "## T\n" })); // another note opened
    const stop2 = caretWatch(view, seen);
    view.dispatch({ selection: { anchor: 2 } });
    expect(seen.mock.calls.at(-1)![0].type).toBe("h2");
    stop2();
    view.destroy();
  });
});

describe("the store writes through the editor as one change", () => {
  it("set patches the frontmatter once, shows it at once, and refuses on a read-only note", () => {
    let text = "# Look\n";
    const applied: unknown[] = [];
    const view = new EditorView({ state: EditorState.create({ doc: text }), parent: document.body });
    const detach = noteStyle.attach({ text: () => text, apply: (c) => { applied.push(c); }, view, run: () => {} });
    expect(noteStyle.set({ "calternal-theme": "nord" })).toBe(true);
    expect(applied).toHaveLength(1);
    expect(noteStyle.page.theme).toBe("nord");
    noteStyle.readOnly = true;
    expect(noteStyle.set({ "calternal-theme": null })).toBe(false);
    noteStyle.readOnly = false;
    detach();
    view.destroy();
    void text;
  });
});
