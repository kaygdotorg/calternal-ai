import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { describe, expect, test, vi } from "vitest";
import { caretFacts, convertChanges, createBlockEditor, toggleBold, typeOfLine } from "../src/block/index";
import { frontmatter, frontmatterMap } from "../src/block/frontmatter";
import { frontmatterReport } from "../src/block/report";
import { prepare } from "../src/block/features";

const NOTE = "---\ntitle: Look\ncalternal-theme: nord\ncalternal-text-size: \"large\"\n---\n# Look\n\nOne thing at a time.\n";
const at = (doc: string, anchor: number, head = anchor) => EditorState.create({ doc, selection: { anchor, head } });

describe("the note's frontmatter as settings", () => {
  test("pairs are read from the block only, unquoted; other lines are skipped", () => {
    const s = at("---\ntitle: Look\n- not a pair\ncalternal-font: mono\nurl: https://a.b/c\n---\n# a: b\n", 0);
    expect(frontmatterMap(s)).toEqual({ title: "Look", "calternal-font": "mono", url: "https://a.b/c" });
    expect(frontmatterMap(at("# a: b\n", 0))).toEqual({});
    expect(frontmatterMap(at(NOTE, 0))["calternal-text-size"]).toBe("large");
  });
  test("the host is told once at build, again only when a change touches the block, never for the body", async () => {
    await prepare(NOTE);
    const seen = vi.fn();
    const parent = document.createElement("div");
    document.body.append(parent);
    const ed = createBlockEditor({ parent, text: NOTE, onFrontmatter: seen });
    expect(seen).toHaveBeenCalledTimes(1);
    expect(seen.mock.calls[0]![0]).toMatchObject({ "calternal-theme": "nord" });
    const body = ed.value.indexOf("One thing");
    for (let i = 0; i < 20; i++) ed.apply({ from: body, insert: "x" }, "user");
    expect(seen).toHaveBeenCalledTimes(1);
    ed.apply({ from: ed.value.indexOf("nord"), to: ed.value.indexOf("nord") + 4, insert: "dracula" }, "command");
    expect(seen).toHaveBeenCalledTimes(2);
    expect(seen.mock.calls[1]![0]["calternal-theme"]).toBe("dracula");
    ed.destroy();
  });
  test("a note with no frontmatter loads nothing and reports nothing; the facet is the only static piece", () => {
    const seen = vi.fn();
    const state = EditorState.create({ doc: "# Plain\n", extensions: [frontmatterReport.of(seen)] });
    expect(state.facet(frontmatterReport)).toBe(seen);
    expect(seen).not.toHaveBeenCalled();
    void frontmatter;
  });
});

describe("block kinds as text", () => {
  test("a line's kind", () => {
    expect(["# a", "## a", "### a", "> a", "```ts", "---", "- a", "1. a", "- [ ] a", "  - [x] a", "plain"].map(typeOfLine)).toEqual(["h1", "h2", "h3", "quote", "code", "divider", "li", "ol", "todo", "todo", "p"]);
  });
  const run = (doc: string, anchor: number, type: Parameters<typeof convertChanges>[1], head = anchor) => {
    const s = at(doc, anchor, head);
    return s.update({ changes: convertChanges(s, type) }).state.doc.toString();
  };
  test("a paragraph becomes a heading, a list item, a to-do, a quote; a heading becomes body again", () => {
    expect(run("Hello\n", 2, "h2")).toBe("## Hello\n");
    expect(run("Hello\n", 2, "li")).toBe("- Hello\n");
    expect(run("Hello\n", 2, "todo")).toBe("- [ ] Hello\n");
    expect(run("## Hello\n", 4, "quote")).toBe("> Hello\n");
    expect(run("## Hello\n", 4, "p")).toBe("Hello\n");
  });
  test("a nested item keeps its indent; several lines convert together", () => {
    expect(run("- a\n  - b\n", 6, "ol")).toBe("- a\n  1. b\n");
    expect(run("a\nb\n", 0, "li", 3)).toBe("- a\n- b\n");
  });
  test("code wraps the lines in a fence and drops their prefixes; the frontmatter is never touched", () => {
    expect(run("- a\n- b\n", 0, "code", 7)).toBe("```\na\nb\n```\n");
    const doc = "---\ncalternal-font: mono\n---\nBody\n";
    expect(run(doc, 1, "h1")).toBe(doc.replace("Body", "# Body"));
  });
  test("bold wraps the selection, takes it back, and opens an empty pair on a bare caret", () => {
    const s = at("some words", 5, 10);
    const on = s.update(toggleBold(s)).state;
    expect(on.doc.toString()).toBe("some **words**");
    expect(caretFacts(on).bold).toBe(true);
    const off = on.update(toggleBold(on)).state;
    expect(off.doc.toString()).toBe("some words");
    const bare = at("ab", 1);
    const pair = bare.update(toggleBold(bare)).state;
    expect(pair.doc.toString()).toBe("a****b");
    expect(pair.update(toggleBold(pair)).state.doc.toString()).toBe("ab");
  });
});

describe("watching the caret costs nothing until someone watches", () => {
  test("the listener exists only for the length of a watch", () => {
    const parent = document.createElement("div");
    document.body.append(parent);
    const ed = createBlockEditor({ parent, text: "# T\n\nbody\n" });
    const seen = vi.fn();
    ed.apply({ from: 0, insert: "x" }, "user");
    expect(seen).not.toHaveBeenCalled();
    const stop = ed.watchCaret(seen);
    expect(seen).toHaveBeenCalledTimes(1);
    ed.view.dispatch({ selection: { anchor: 5 } });
    expect(seen).toHaveBeenCalledTimes(2);
    stop();
    ed.view.dispatch({ selection: { anchor: 6 } });
    ed.apply({ from: 0, insert: "y" }, "user");
    expect(seen).toHaveBeenCalledTimes(2);
    ed.convert("h2");
    expect(ed.value.startsWith("## ")).toBe(true);
    ed.destroy();
    void EditorView;
  });
});
