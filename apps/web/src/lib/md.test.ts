import { describe, expect, it } from "vitest";
import { inline, parse } from "./md";

describe("markdown subset", () => {
  it("parses inline code, bold, italic and https links; not other schemes", () => {
    expect(inline("a `b` **c** *d* [e](https://x.y/z) [f](javascript:alert(1))").map((x) => x.t)).toEqual(["text", "code", "text", "b", "text", "i", "text", "a", "text"]);
    expect(inline("[f](javascript:alert(1))").some((x) => x.t === "a")).toBe(false);
  });
  it("parses blocks", () => {
    const b = parse("# T\n\ntext one\nstill one\n\n- a\n- b\n\n1. x\n2. y\n\n```rust\nlet a = 1;\n```");
    expect(b.map((x) => x.t)).toEqual(["h", "p", "ul", "ol", "code"]);
    expect(b[4]).toMatchObject({ lang: "rust", v: "let a = 1;" });
  });
  it("closes an unfinished code fence at the end of a streaming message", () => {
    expect(parse("```ts\nlet a")).toEqual([{ t: "code", lang: "ts", v: "let a" }]);
  });
});
