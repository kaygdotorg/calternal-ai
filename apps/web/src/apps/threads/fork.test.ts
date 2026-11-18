import { describe, expect, it } from "vitest";
import { addVersion, newFork, stamp, switchTo, tailAt } from "./fork";
import { FixtureClient } from "./fixtures";
import type { Message } from "./model";

const msg = (id: string, text = id): Message => ({ id, role: "user", at: 0, blocks: [{ type: "text", text }] });

describe("fork rules", () => {
  it("a new fork shows the edited message as version 2 of 2 and keeps the original's length", () => {
    const f = newFork(6, 20, msg("e1"));
    expect(f.active).toBe(1);
    expect(f.saved).toBe(20);
    expect(tailAt(f, 6)?.id).toBe("e1");
    expect(tailAt(f, 5)).toBeUndefined();
    expect(stamp(f, 6, msg("e1")).version).toEqual({ n: 2, of: 2 });
    expect(stamp(f, 7, msg("x")).version).toBeUndefined();
  });
  it("switching back restores the original count; switching to a version counts its own tail", () => {
    const f = newFork(6, 20, msg("e1"));
    f.tails[1]!.push(msg("a1"), msg("a2"));
    expect(switchTo(f, 0, 9)).toBe(20);
    expect(tailAt(f, 6)).toBeUndefined();
    addVersion(f, 20, msg("e2"));
    expect(f.active).toBe(2);
    expect(switchTo(f, 1, 7)).toBe(6 + 3);
    expect(switchTo(f, 5, 9)).toBe(6 + 1); // clamped to the last version
  });
});

describe("the fixture client's edit", () => {
  it("an edit keeps the old tail, answers on the new branch, and one version shows at a time", async () => {
    const c = new FixtureClient();
    const id = "t-idle";
    const before = (await c.getThread(id))!.messageCount;
    const i = (await c.messages(id, 0, before)).findIndex((m) => m.role === "user");
    const target = (await c.messages(id, i, i + 1))[0]!;
    await c.editMessage!(id, target.id, "A different question", "r1");
    const after = await c.messages(id, 0, 200);
    expect(after.length).toBe(i + 1);
    expect(after[i]!.version).toEqual({ n: 2, of: 2 });
    expect(after[i]!.blocks[0]).toMatchObject({ type: "text", text: "A different question" });
    await c.setVersion!(id, target.id, 1);
    const orig = await c.messages(id, 0, 500);
    expect(orig.length).toBe(before);
    expect(orig[i]!.version).toEqual({ n: 1, of: 2 });
    expect(orig[i]!.id).toBe(target.id);
  });
  it("editing the same message again adds a version", async () => {
    const c = new FixtureClient();
    const id = "t-idle";
    const ms = await c.messages(id, 0, 50);
    const i = ms.findIndex((m) => m.role === "user");
    await c.editMessage!(id, ms[i]!.id, "one", "r1");
    const first = (await c.messages(id, i, i + 1))[0]!;
    await c.editMessage!(id, first.id, "two", "r2");
    const now = (await c.messages(id, i, i + 1))[0]!;
    expect(now.version).toEqual({ n: 3, of: 3 });
    await c.setVersion!(id, now.id, 2);
    expect(((await c.messages(id, i, i + 1))[0]!.blocks[0] as { text: string }).text).toBe("one");
  });
});
