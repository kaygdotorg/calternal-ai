import { beforeAll, describe, expect, test } from "vitest";
import "../apps/settings/registry";
import { plan, setItems } from "./engine";
import { buildIndex, search, type Item } from "./search";

const item = (i: number, title: string, space = "Personal"): Item => ({ id: `t${i}`, kind: "thread", title, space });
const ITEMS: Item[] = [item(1, "Split store.rs by concern"), item(2, "Journal replay is slow after restart"), item(3, "Expired invite test"), item(4, "Mesh idle-peer RSS guard", "Infra"), item(5, "Conformance run on dev VMs")];

describe("search", () => {
  const ix = buildIndex(ITEMS);
  test("finds by word, prefix and inside a word", () => {
    expect(search(ix, "journal")[0]?.id).toBe("t2");
    expect(search(ix, "conf")[0]?.id).toBe("t5");
    expect(search(ix, "peer")[0]?.id).toBe("t4");
  });
  test("tolerates one typo in words of four letters or more", () => {
    expect(search(ix, "jurnal")[0]?.id).toBe("t2");
    expect(search(ix, "expierd")[0]?.id).toBe("t3");
    expect(search(ix, "jrnl")).toEqual([]);
  });
  test("every word must match", () => {
    expect(search(ix, "journal invite")).toEqual([]);
    expect(search(ix, "split concern")[0]?.id).toBe("t1");
  });
  // The speed of a keystroke (2,000 titles under the 8 ms budget) is measured where a wall clock is meaningful: `e2e/palette.mjs` at 6x CPU slowdown and the perf probes. A unit test that asserted
  // milliseconds failed under load (16 ms once on a busy host) while nothing was wrong, so this one asserts the work instead: a big index answers with a bounded, correct page.
  test("2,000 titles: a bounded page, best first, from one index", () => {
    const big = buildIndex(Array.from({ length: 2000 }, (_, i) => item(i, `thread ${i} about storage layout and journal replay ${i % 17}`)));
    const hits = search(big, "journal replay");
    expect(hits.length).toBeGreaterThan(0);
    expect(hits.length).toBeLessThanOrEqual(12);
    expect(search(big, "journal replay", 3)).toHaveLength(3);
    expect(search(big, "journal replay").map((h) => h.id)).toEqual(hits.map((h) => h.id)); // the index is not consumed by a query
  });
});

describe("plan", () => {
  beforeAll(() => setItems(ITEMS, "Personal"));
  test("says what Enter does and always offers Ask last", () => {
    const p = plan("journal");
    expect(p.intent).toBe("Open: Journal replay is slow after restart");
    expect(p.rows.at(-1)?.kind).toBe("ask");
  });
  test("a phrase with a date is a task", () => {
    const p = plan("call plumber friday 5pm");
    expect(p.intent).toMatch(/^New task: call plumber/);
    expect(p.parsed.spans.some((s) => s.type === "date")).toBe(true);
  });
  test("commands from plugin data are found by their extra words", () => {
    const hit = plan("wallpaper").rows.find((r) => r.label === "Wallpaper and fonts");
    expect(hit?.action.go).toBe("/settings/appearance/appearance.wallpaper");
  });
  test("results from another space say which", () => {
    expect(plan("mesh").rows[0]?.space).toBe("Infra");
  });
  test("nothing matches: the line is a question for the agent", () => {
    expect(plan("why is the sky blue", "Sonnet 5.5").intent).toBe("Ask Sonnet 5.5: why is the sky blue");
  });
  test("an empty line has no rows", () => {
    expect(plan("   ").rows).toEqual([]);
  });
});
