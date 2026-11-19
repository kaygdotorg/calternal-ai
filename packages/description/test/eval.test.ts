import { describe, expect, it } from "vitest";
import { holds, iconOf, marksOf, titleOf } from "../src";
import type { Marks, RowTemplate } from "../src";

describe("row template evaluation", () => {
  const t: RowTemplate = { title: { field: "title", fallback: "Untitled" } };
  it("titles from the field, else the fallback, else empty, and never drops the row", () => {
    expect(titleOf(t, { title: "Plan" })).toBe("Plan");
    expect(titleOf(t, { title: "" })).toBe("Untitled");
    expect(titleOf({ title: { field: "title" } }, {})).toBe("");
  });
  it("predicates compare literals on the row's own fields; operands only the host can fill never hold", () => {
    expect(holds({ field: "read", op: "eq", value: { literal: true } }, { read: true })).toBe(true);
    expect(holds({ field: "n", op: "gt", value: { literal: 2 } }, { n: 3 })).toBe(true);
    expect(holds({ field: "n", op: "lte", value: { literal: 2 } }, { n: 3 })).toBe(false);
    expect(holds({ field: "s", op: "prefix", value: { literal: "ab" } }, { s: "abc" })).toBe(true);
    expect(holds({ field: "s", op: "in", value: { literal: ["a", "b"] } }, { s: "b" })).toBe(true);
    expect(holds({ field: "s", op: "exists" }, { s: 0 })).toBe(true);
    expect(holds({ field: "person", op: "eq", value: { viewer: "person" } }, { person: "p" })).toBe(false);
    expect(holds(undefined, {})).toBe(true);
  });
  it("marks: at most three, only where `when` holds, zero counts and empty text draw nothing", () => {
    const marks: Marks = [
      { kind: "icon", icon: "check", tone: "good", when: { field: "read", op: "eq", value: { literal: true } } },
      { kind: "count", field: "n" }, { kind: "text", field: "due", tone: "warn" }, { kind: "dot", field: "unread" },
    ];
    expect(marksOf(marks, { read: false, n: 0, due: "", unread: false })).toEqual([]);
    expect(marksOf(marks, { read: true, n: 4, due: "Fri", unread: true })).toEqual([{ kind: "icon", icon: "check", tone: "good" }, { kind: "count", text: "4", tone: undefined }, { kind: "text", text: "Fri", tone: "warn" }]);
  });
  it("icons: a name, a field through a map (unmapped draws the kind's), the kind's own, or none", () => {
    expect(iconOf(undefined, {}, "notes")).toBeUndefined();
    expect(iconOf("star", {}, "notes")).toBe("star");
    expect(iconOf({ kindIcon: true }, {}, "notes")).toBe("notes");
    expect(iconOf({ field: "provider", map: { claude: "claude" } }, { provider: "claude" }, "threads")).toBe("claude");
    expect(iconOf({ field: "provider", map: { claude: "claude" } }, { provider: "other" }, "threads")).toBe("threads");
  });
});

import { secondaryOf, relativeDate, imageIconOf } from "../src/render/eval";
describe("secondary line (revision 13.1)", () => {
  const now = Date.parse("2026-11-19T12:00:00Z");
  const parts = [{ field: "updated", format: "relative_date" as const }, { field: "snippet", format: "text" as const }];
  it("joins present parts with a middle dot, formats relative dates on the device clock, omits an absent part and its separator", () => {
    expect(secondaryOf(parts, { updated: "2026-11-19T10:00:00Z", snippet: "Hello  world" }, now)).toBe("2 hours ago · Hello world");
    expect(secondaryOf(parts, { updated: (now - 86400_000) / 1000, snippet: "" }, now)).toBe("yesterday");
    expect(secondaryOf(parts, { snippet: "only" }, now)).toBe("only");
    expect(secondaryOf(parts, {}, now)).toBe("");
  });
  it("a template without the declaration has one line; short_date formats the day", () => {
    expect(secondaryOf(undefined, { a: 1 }, now)).toBeUndefined();
    expect(secondaryOf([{ field: "d", format: "short_date" }], { d: "2026-11-19T10:00:00Z" }, now)).toMatch(/Nov|19/);
    expect(relativeDate(now - 3 * 86400_000, now)).toMatch(/3 days ago/);
  });
  it("reads the image icon declaration", () => {
    expect(imageIconOf({ image: { field: "thumbnail", fallback: "snippet" } })).toEqual({ field: "thumbnail", fallback: "snippet" });
    expect(imageIconOf("notes")).toBeNull();
  });
});
