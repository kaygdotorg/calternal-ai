import { describe, expect, test } from "vitest";
import { marksOf } from "./engine";
import { mentionPool, pick, skillPool, triggerAt } from "./mentions";

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

describe("where the caret is", () => {
  test("after @ and some letters", () => expect(triggerAt("hello @pr", 9)).toEqual({ kind: "mention", query: "pr", start: 6 }));
  test("not inside an email", () => expect(triggerAt("write a@b", 9)).toBeNull());
  test("a / at the start of a line is a skill, mid-line it is not", () => {
    expect(triggerAt("/rev", 4)).toEqual({ kind: "skill", query: "rev", start: 0 });
    expect(triggerAt("a and/or b", 6)).toBeNull();
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
