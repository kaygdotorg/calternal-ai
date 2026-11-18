import { describe, expect, it } from "vitest";
import { cssMs, tokenMs } from "../src";

describe("motion tokens", () => {
  it("reads milliseconds and seconds, and falls back on anything else", () => {
    expect(cssMs("340ms", 1)).toBe(340);
    expect(cssMs(" 0.19s ", 1)).toBeCloseTo(190);
    expect(cssMs(".22s", 1)).toBeCloseTo(220);
    expect(cssMs("200", 1)).toBe(200);
    expect(cssMs("", 7)).toBe(7);
    expect(cssMs("var(--x)", 7)).toBe(7);
  });
  it("a token the browser normalised to seconds is read as milliseconds", () => {
    document.documentElement.style.setProperty("--dur-test", ".22s");
    expect(tokenMs("--dur-test", 1)).toBeCloseTo(220);
    expect(tokenMs("--dur-missing", 99)).toBe(99);
  });
});
