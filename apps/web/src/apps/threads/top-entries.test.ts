import { describe, expect, it } from "vitest";
import { DEFAULT_ENTRIES, fromMarkdown, toMarkdown } from "./top-entries.svelte";

describe("the top entries file", () => {
  it("round-trips", () => {
    const list = [{ target: "new-thread", label: "New thread" }, { target: "thread:abc", label: "Release: checklist" }];
    expect(fromMarkdown(toMarkdown(list))).toEqual(list);
  });
  it("gives the defaults for no file, another kind of file, or nothing readable", () => {
    expect(fromMarkdown(null)).toEqual(DEFAULT_ENTRIES);
    expect(fromMarkdown("hello")).toEqual(DEFAULT_ENTRIES);
    expect(fromMarkdown("---\nkind: threads-sidebar-top\n---\n")).toEqual(DEFAULT_ENTRIES);
  });
  it("drops unknown targets and keeps an emptied list empty", () => {
    expect(fromMarkdown("---\nkind: threads-sidebar-top\n---\n- bogus: x\n- new-thread: Go\n")).toEqual([{ target: "new-thread", label: "Go" }]);
    expect(fromMarkdown("---\nkind: threads-sidebar-top\n---\n- bogus: x\n")).toEqual([]);
  });
});
