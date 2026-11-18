import { describe, expect, it, vi } from "vitest";
import { capitalise, copyText, shortDate } from "./format";

describe("format helpers", () => {
  it("shows a date in the locale, and the fallback for nothing", () => {
    expect(shortDate(Date.UTC(2026, 10, 17, 12))).toMatch(/2026/);
    expect(shortDate(null)).toBe("never");
    expect(shortDate(undefined, "unknown")).toBe("unknown");
  });
  it("capitalises one word and leaves nothing alone", () => {
    expect(capitalise("editor")).toBe("Editor");
    expect(capitalise("")).toBe("");
  });
  it("says whether the copy worked, and never throws", async () => {
    const write = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText: write }, configurable: true });
    expect(await copyText("x")).toBe(true);
    write.mockRejectedValue(new Error("denied"));
    expect(await copyText("x")).toBe(false);
  });
});
