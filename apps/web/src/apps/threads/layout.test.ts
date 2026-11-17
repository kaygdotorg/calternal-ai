import { beforeEach, describe, expect, it } from "vitest";

describe("deleting a section can be undone", () => {
  beforeEach(() => localStorage.clear());
  it("puts the section back where it was with its threads", async () => {
    const { layout } = await import("./layout.svelte");
    const a = layout.create("A"), b = layout.create("B");
    layout.sections[0].threadIds = ["t1", "t2"];
    const undo = layout.remove(a.id)!;
    expect(layout.sections.map((s) => s.id)).toEqual([b.id]);
    expect(layout.loose).toContain("t1");
    expect(undo()).toBe(true);
    expect(layout.sections.map((s) => s.id)).toEqual([a.id, b.id]);
    expect(layout.sections[0].threadIds).toEqual(["t1", "t2"]);
    expect(layout.loose).not.toContain("t1");
    expect(undo()).toBe(false);
  });
});
