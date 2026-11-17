import { describe, expect, test } from "vitest";
import { marksOf } from "./engine";
import { blockPool, mentionPool, notePool, pick, skillPool, tagPool } from "./mentions";

const pool = mentionPool(["Claude Code", "Codex"], [{ id: "t1", title: "Split store.rs by concern" }, { id: "t2", title: "Expired invite test" }], [{ id: "n1", title: "Storage layout" }]);

describe("the @ picker", () => {
  test("groups by kind in a fixed order: people, agents, threads, items", () => {
    expect(pick(pool, "").map((g) => g.kind)).toEqual(["person", "agent", "thread", "item"]);
  });
  test("ranks by the same typo-tolerant search", () => {
    expect(pick(pool, "pria")[0]?.entries[0]?.label).toBe("Priya");
    expect(pick(pool, "expierd")[0]?.entries[0]?.label).toBe("Expired invite test");
  });
  test("a title with spaces is inserted bracketed", () => {
    expect(pool.find((m) => m.label === "Expired invite test")?.insert).toBe("@[Expired invite test]");
    expect(pool.find((m) => m.label === "Priya")?.insert).toBe("@Priya");
  });
  test("/ lists skills", () => {
    expect(pick(skillPool(), "rev")[0]?.entries[0]?.label).toBe("Code review");
  });
});

describe("marks (shared by the palette and the composer)", () => {
  test("recognised words, mentions and skills, without overlap", () => {
    const m = marksOf("/review call @Priya friday 5pm !high");
    expect(m.map((x) => x.kind)).toEqual(expect.arrayContaining(["skill", "date", "time", "priority"]));
    expect(m.some((x) => x.kind === "mention" || x.kind === "person")).toBe(true);
    for (let i = 1; i < m.length; i++) expect(m[i]!.start).toBeGreaterThanOrEqual(m[i - 1]!.end);
  });
  test("quoted text is never read", () => {
    expect(marksOf('call "friday" @[Storage layout]').some((x) => x.kind === "date")).toBe(false);
  });
});

describe("note pickers", () => {
  test("[[ lists notes and inserts the link", () => {
    const g = pick(notePool([{ id: "n1", title: "Storage layout" }]), "stor");
    expect(g[0]!.entries[0]!.insert).toBe("[[Storage layout]]");
  });
  test("# offers the typed word as a new tag only when no tag starts with it", () => {
    expect(tagPool(["design"], "des").some((m) => m.sub === "New tag")).toBe(false);
    expect(tagPool(["design"], "ops")[0]!.insert).toBe("#ops");
  });
  test("/ lists blocks; the code block puts the caret inside the fence", () => {
    const code = pick(blockPool(), "code")[0]!.entries[0]!;
    expect(code.insert).toBe("```\n\n```");
    expect(code.caret).toBe(4);
  });
});
