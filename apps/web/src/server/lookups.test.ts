import { afterEach, describe, expect, it, vi } from "vitest";
import { server } from "./client";
import { hitOf, lookups } from "./lookups";
import { remoteMentionPool, remoteSkillPool, pick } from "../palette/mentions";

afterEach(() => vi.restoreAllMocks());

describe("message search over search.query", () => {
  it("maps a hit to a result row: the thread from the address, the excerpt as the line with the first match marked", () => {
    const h = hitOf({ address: "/threads/t-1", kind: "threads", space_id: "s", title: "Split store.rs", excerpt: [{ text: "  the journal is ", matched: false }, { text: "read", matched: true }, { text: " twice", matched: false }] });
    expect(h.threadId).toBe("t-1");
    expect(h.threadTitle).toBe("Split store.rs");
    expect(h.line).toBe("the journal is read twice");
    expect(h.line.slice(h.start, h.end)).toBe("read");
    expect(h.messageId).toBeUndefined();
    expect(h.at).toBeUndefined();
  });
  it("carries the message id and time of a message hit, and the partial flag of the page", async () => {
    const hit = { address: "/threads/t-1", kind: "threads", space_id: "s", title: "T", excerpt: [{ text: "read", matched: true }], message_id: "0194-aaaa", message_at: "2026-11-18T12:00:00+05:30" };
    expect(hitOf(hit as never)).toMatchObject({ messageId: "0194-aaaa", at: Date.parse("2026-11-18T12:00:00+05:30") });
    vi.spyOn(server, "operate").mockResolvedValue({ hits: [hit], partial: true } as never);
    expect(await lookups(() => "s").searchMessages("read")).toMatchObject({ partial: true, hits: [{ messageId: "0194-aaaa" }] });
  });
  it("asks for threads only, at most 50, and passes nothing else", async () => {
    const op = vi.spyOn(server, "operate").mockResolvedValue({ hits: [] } as never);
    await lookups(() => "space-1").searchMessages("journal", 500);
    expect(op).toHaveBeenCalledWith("search.query", { query: "journal", kinds: ["threads"], limit: 50 });
  });
});

describe("the composer's pickers over the lookups", () => {
  it("people.lookup and skills.list are asked in the current space, by prefix", async () => {
    const op = vi.spyOn(server, "operate").mockResolvedValueOnce({ people: [{ id: "u1", kind: "account", name: "Priya" }] } as never).mockResolvedValueOnce({ skills: [{ name: "review", description: "Code review" }] } as never);
    const l = lookups(() => "space-9");
    expect(await l.people("Pr")).toEqual([{ id: "u1", name: "Priya", kind: "account" }]);
    expect(await l.skills("re")).toEqual([{ name: "review", description: "Code review" }]);
    expect(op).toHaveBeenNthCalledWith(1, "people.lookup", { space_id: "space-9", prefix: "Pr" });
    expect(op).toHaveBeenNthCalledWith(2, "skills.list", { space_id: "space-9", prefix: "re" });
  });
  it("threads.lookup and items.lookup have no space (they search every readable space)", async () => {
    const op = vi.spyOn(server, "operate").mockResolvedValue({ hits: [{ address: "/notes/n-4", kind: "notes", space_id: "s", title: "Storage layout", excerpt: [] }] } as never);
    const rows = await lookups(() => "x").items("Sto");
    expect(rows).toEqual([{ id: "n-4", title: "Storage layout", kind: "notes", space: "s" }]);
    expect(op).toHaveBeenCalledWith("items.lookup", { prefix: "Sto", limit: 8 });
  });
  it("the pools hold only what the server found (no fixture people), grouped by kind", () => {
    const pool = remoteMentionPool(["Claude Code"], { people: [{ id: "u1", name: "Priya Rao", kind: "account" }], threads: [{ id: "t1", title: "Split store" }], items: [{ id: "n1", title: "Storage layout", kind: "notes" }] });
    const groups = pick(pool, "");
    expect(groups.map((g) => g.kind)).toEqual(["person", "agent", "thread", "item"]);
    expect(pool.find((m) => m.kind === "person")!.insert).toBe("@[Priya Rao]");
    expect(pool.some((m) => m.label === "Kay")).toBe(false);
    expect(remoteSkillPool([{ name: "review", description: "Code review" }])[0]).toMatchObject({ kind: "skill", insert: "/review ", sub: "Code review" });
  });
});
