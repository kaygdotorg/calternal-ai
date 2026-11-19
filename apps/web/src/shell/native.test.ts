import { beforeEach, describe, expect, test, vi } from "vitest";
import { haptic } from "@calternal/ui";

// The bridge exists only inside the iPhone shell: every call is behind a check, so a browser is unchanged; inside it, haptics are not sent in bursts and can be switched off.
type W = { calternalNative?: { haptic?: ReturnType<typeof vi.fn>; appearance?: ReturnType<typeof vi.fn>; ready?: ReturnType<typeof vi.fn> } };
const w = window as unknown as W;

describe("haptics and the native bridge", () => {
  beforeEach(() => { delete w.calternalNative; localStorage.clear(); vi.resetModules(); });
  test("in a browser (no bridge) a haptic does nothing and does not throw", () => {
    expect(() => haptic("success")).not.toThrow();
  });
  test("inside the shell it is sent once, and not again within 80 ms", async () => {
    const h = vi.fn().mockResolvedValue(undefined);
    w.calternalNative = { haptic: h };
    const m = await import("@calternal/ui");
    vi.spyOn(performance, "now").mockReturnValueOnce(1000).mockReturnValueOnce(1030).mockReturnValueOnce(1200);
    m.haptic("selection"); m.haptic("selection"); m.haptic("success");
    expect(h.mock.calls.map((c) => c[0])).toEqual(["selection", "success"]);
  });
  test("it can be switched off, and a failing bridge is ignored", async () => {
    const h = vi.fn().mockRejectedValue(new Error("no"));
    w.calternalNative = { haptic: h };
    const m = await import("@calternal/ui");
    localStorage.setItem("cal.haptics", "0");
    m.haptic("error");
    expect(h).not.toHaveBeenCalled();
    localStorage.removeItem("cal.haptics");
    vi.spyOn(performance, "now").mockReturnValue(5000);
    expect(() => m.haptic("error")).not.toThrow();
  });
  test("appearance is reported once per change, and ready once", async () => {
    const a = vi.fn().mockResolvedValue(undefined), r = vi.fn().mockResolvedValue(undefined);
    w.calternalNative = { appearance: a, ready: r };
    const n = await import("./native");
    n.nativeAppearance("dark", "#101010"); n.nativeAppearance("dark", "#101010"); n.nativeAppearance("light", "#f4f4f4");
    n.nativeReady(); n.nativeReady();
    expect(a.mock.calls.map((c) => c[0])).toEqual([{ scheme: "dark", background: "#101010" }, { scheme: "light", background: "#f4f4f4" }]);
    expect(r).toHaveBeenCalledTimes(1);
  });
});
