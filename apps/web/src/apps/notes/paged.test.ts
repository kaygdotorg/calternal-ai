import { describe, expect, it } from "vitest";
import type { Page } from "./client";
import { Paged } from "./paged.svelte";

const rows = (n: number) => Array.from({ length: n }, (_, i) => ({ id: `n${i}` }));
/** A reader over `all` with pages of 3; counts its calls and can be told to refuse a cursor. */
function source(all: { id: string }[], opts: { refuse?: () => boolean } = {}) {
  const calls: (string | undefined)[] = [];
  const read = async (cursor?: string): Promise<Page<{ id: string }>> => {
    calls.push(cursor);
    if (cursor && opts.refuse?.()) throw new Error("cursor");
    const from = cursor ? Number(cursor) : 0;
    return { items: all.slice(from, from + 3), next: from + 3 < all.length ? String(from + 3) : null };
  };
  return { read, calls };
}

describe("Paged", () => {
  it("reads one page, then the next on demand, to the end", async () => {
    const s = source(rows(7)); const p = new Paged(s.read, (r) => r.id);
    await p.refresh(); expect(p.items).toHaveLength(3); expect(p.more).toBe(true);
    await p.next(); await p.next(); expect(p.items).toHaveLength(7); expect(p.more).toBe(false);
    await p.next(); expect(s.calls).toEqual([undefined, "3", "6"]); // nothing more is asked for
  });
  it("keeps the pages scrolled past when the first page is read again, without listing a moved row twice", async () => {
    const all = rows(7); const s = source(all); const p = new Paged(s.read, (r) => r.id);
    await p.refresh(); await p.next();
    all.unshift(...all.splice(5, 1)); // n5 (not yet shown) moves to the top; the order is n5 n0 n1 n2 ...
    await p.refresh();
    expect(p.items.map((r) => r.id)).toEqual(["n5", "n0", "n1", "n2", "n3", "n4"]);
  });
  it("a row dropped locally goes at once", async () => {
    const p = new Paged(source(rows(5)).read, (r) => r.id); await p.refresh(); p.drop("n1"); expect(p.items.map((r) => r.id)).toEqual(["n0", "n2"]);
  });
  it("a refused cursor starts over once, and a second failure stops with the words and waits for retry", async () => {
    let refuse = true; const s = source(rows(7), { refuse: () => refuse }); const p = new Paged(s.read, (r) => r.id, () => "could not load");
    await p.refresh(); await p.next(); // first refusal: start over (3 rows again)
    expect(p.items).toHaveLength(3); expect(p.error).toBe("");
    await p.next(); expect(p.error).toBe("could not load");
    const before = s.calls.length; await p.next(); expect(s.calls.length).toBe(before); // stalled: no spinning
    refuse = false; await p.retry(); expect(p.items).toHaveLength(6); expect(p.error).toBe("");
  });
  it("an older read finishing late does not overwrite a newer one", async () => {
    let release!: () => void; const gate = new Promise<void>((r) => (release = r)); let n = 0;
    const read = async () => { const mine = ++n; if (mine === 1) await gate; return { items: [{ id: mine === 1 ? "old" : "new" }], next: null }; };
    const p = new Paged(read, (r) => r.id); const first = p.refresh(); await p.refresh(); release(); await first;
    expect(p.items.map((r) => r.id)).toEqual(["new"]);
  });
});
