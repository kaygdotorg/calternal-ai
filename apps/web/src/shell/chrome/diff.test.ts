import { describe, expect, test } from "vitest";
import { countsOnly, selectionEchoOnly } from "./diff";
import type { ChromeDescription } from "./types";

const base = (): ChromeDescription => ({
  version: 1, revision: 1, scheme: "dark", header: { title: "A" },
  sidebar: {
    open: false, title: "C", selectors: [{ id: "f", selected: "a", options: [{ id: "a", title: "A", badge: { kind: "count", value: 3 } }] }],
    sections: [{ id: "s", title: null, count: 100, rows: [{ id: "r1", title: "One", selected: true, trailing: [{ kind: "count", value: 2 }] }, { id: "r2", title: "Two" }] }],
    apps: [], account: { id: "account", initial: "K", title: "Kay" }, search: { placeholder: "Find" },
  },
});

describe("selection echo", () => {
  test("only the selected flag moving to the row the shell chose is an echo", () => {
    const next = base(); next.revision = 9; next.sidebar.sections[0]!.rows[0]!.selected = false; next.sidebar.sections[0]!.rows[1]!.selected = true;
    expect(selectionEchoOnly(base(), next, "r2")).toBe(true);
  });
  test("a different row selected, a changed title or any other change is not", () => {
    const other = base(); other.sidebar.sections[0]!.rows[1]!.selected = true;
    expect(selectionEchoOnly(base(), other, "r1")).toBe(false); // r2 is selected but the shell chose r1
    const title = base(); title.sidebar.sections[0]!.rows[0]!.selected = false; title.sidebar.sections[0]!.rows[1]!.selected = true; title.header.title = "Two";
    expect(selectionEchoOnly(base(), title, "r2")).toBe(false);
    const row = base(); row.sidebar.sections[0]!.rows[1]!.title = "Renamed"; row.sidebar.sections[0]!.rows[1]!.selected = true;
    expect(selectionEchoOnly(base(), row, "r2")).toBe(false);
  });
});

describe("counts only", () => {
  test("a count badge or a section count moving is counts only", () => {
    const next = base(); next.revision = 4; next.sidebar.sections[0]!.count = 101; next.sidebar.sections[0]!.rows[0]!.trailing![0]!.value = 5; next.sidebar.selectors[0]!.options[0]!.badge!.value = 9;
    expect(countsOnly(base(), next)).toBe(true);
  });
  test("a row, a title or a selection changing with it is not", () => {
    const a = base(); a.sidebar.sections[0]!.count = 101; a.sidebar.sections[0]!.rows[1]!.title = "x";
    expect(countsOnly(base(), a)).toBe(false);
    const b = base(); b.sidebar.sections[0]!.rows[0]!.selected = false;
    expect(countsOnly(base(), b)).toBe(false);
  });
});
