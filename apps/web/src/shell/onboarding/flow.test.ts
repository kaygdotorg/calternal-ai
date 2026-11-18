import { describe, expect, it } from "vitest";
import { after, before, clear, load, path, save, skippable } from "./flow";

const store = () => { const m = new Map<string, string>(); return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v), removeItem: (k: string) => void m.delete(k) }; };

describe("onboarding path", () => {
  it("one path for everyone, and three more steps for someone who writes code", () => {
    expect(path(false)).toEqual(["you", "look", "machine", "provider", "done"]);
    expect(path(true)).toEqual(["you", "look", "machine", "provider", "repos", "cli", "direct", "done"]);
  });
  it("moves forward and back along the path", () => {
    expect(after(false, "provider")).toBe("done"); expect(after(true, "provider")).toBe("repos"); expect(after(true, "direct")).toBe("done"); expect(after(false, "done")).toBeNull();
    expect(before(false, "machine")).toBe("look"); expect(before(true, "you")).toBeNull();
  });
  it("only what is safe to skip can be skipped", () => {
    expect(skippable.you).toBe(false); expect(skippable.done).toBe(false);
    expect(skippable.look && skippable.machine && skippable.provider).toBe(true);
  });
});
describe("resuming", () => {
  it("saves and loads where the person is, and drops what is not on the path", () => {
    const s = store(); save({ who: "invited", coder: false, step: "look", done: ["you", "repos"], skipped: [] }, s);
    expect(load(s)).toEqual({ who: "invited", coder: false, step: "look", done: ["you"], skipped: [] });
    clear(s); expect(load(s)).toBeNull();
  });
  it("a damaged or foreign value is nothing", () => {
    const s = store(); s.setItem("cal.onboarding", "{oops"); expect(load(s)).toBeNull();
    s.setItem("cal.onboarding", JSON.stringify({ who: "owner", step: "elsewhere" })); expect(load(s)).toBeNull();
  });
});
