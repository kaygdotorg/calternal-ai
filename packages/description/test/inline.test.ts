import { describe, expect, it } from "vitest";
import { parseInline } from "../src/components/inline";

describe("inline markup (the closed subset)", () => {
  it("splits emphasis, strong text, code and explicit links", () => {
    expect(parseInline("a **b** *c* `d` [e](https://x.test/p?q=1) f")).toEqual([
      { t: "text", text: "a " }, { t: "strong", text: "b" }, { t: "text", text: " " }, { t: "em", text: "c" }, { t: "text", text: " " }, { t: "code", text: "d" }, { t: "text", text: " " },
      { t: "link", text: "e", href: "https://x.test/p?q=1" }, { t: "text", text: " f" }]);
  });
  it("never makes a link of a non-http(s) address, a bare URL, HTML or an image", () => {
    for (const s of ["[x](javascript:alert(1))", "https://bare.test", "<b>x</b>", "![i](https://x.test/a.png)"]) expect(parseInline(s).some((x) => x.t === "link" && !/^https?:/.test(x.href))).toBe(false);
    expect(parseInline("https://bare.test")).toEqual([{ t: "text", text: "https://bare.test" }]);
    expect(parseInline("<b>x</b>")).toEqual([{ t: "text", text: "<b>x</b>" }]);
  });
  it("leaves unmatched markers and snake_case words alone", () => {
    expect(parseInline("a * b and snake_case_name")).toEqual([{ t: "text", text: "a * b and snake_case_name" }]);
  });
});
