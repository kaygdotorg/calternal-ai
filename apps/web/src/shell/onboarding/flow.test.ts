import { describe, expect, it } from "vitest";
import { after, before, clear, load, path, save, skippable } from "./flow";

const store = () => { const m = new Map<string, string>(); return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v), removeItem: (k: string) => void m.delete(k) }; };

describe("onboarding path", () => {
  it("the owner has five steps, an invited person three", () => {
    expect(path("owner")).toEqual(["you", "look", "machine", "provider", "done"]);
    expect(path("invited")).toEqual(["you", "look", "done"]);
  });
  it("moves forward and back along the path", () => {
    expect(after("owner", "look")).toBe("machine"); expect(after("invited", "look")).toBe("done"); expect(after("owner", "done")).toBeNull();
    expect(before("owner", "machine")).toBe("look"); expect(before("invited", "you")).toBeNull();
  });
  it("only what is safe to skip can be skipped", () => {
    expect(skippable.you).toBe(false); expect(skippable.done).toBe(false);
    expect(skippable.look && skippable.machine && skippable.provider).toBe(true);
  });
});
describe("resuming", () => {
  it("saves and loads where the person is, and drops what is not on the path", () => {
    const s = store(); save({ who: "invited", step: "look", done: ["you", "machine"], skipped: [] }, s);
    expect(load(s)).toEqual({ who: "invited", step: "look", done: ["you"], skipped: [] });
    clear(s); expect(load(s)).toBeNull();
  });
  it("a damaged or foreign value is nothing", () => {
    const s = store(); s.setItem("cal.onboarding", "{oops"); expect(load(s)).toBeNull();
    s.setItem("cal.onboarding", JSON.stringify({ who: "owner", step: "elsewhere" })); expect(load(s)).toBeNull();
  });
});
