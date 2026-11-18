import { describe, expect, it, test } from "vitest";
import { inline, parse, parseMore, type Parsed } from "./md";

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

describe("parseMore", () => {
  const sample = "First paragraph with `code` and **bold**.\n\n- one\n- two\n\n```ts\nlet a = 1;\n\nlet b = 2;\n```\n\nA [link](https://example.org/x) here\ncontinues.\n\n1. first\n2. second\n\n# Heading\n\nLast words";
  test("every prefix of a growing text gives the same blocks as a whole parse", () => {
    let prev: Parsed | undefined;
    for (let i = 0; i <= sample.length; i++) { prev = parseMore(prev, sample.slice(0, i)); expect(prev.blocks).toEqual(parse(sample.slice(0, i))); }
  });
  test("text that did not just grow is parsed whole", () => {
    const a = parseMore(undefined, "One.\n\nTwo.\n\nThree");
    expect(parseMore(a, "Other.\n\nText").blocks).toEqual(parse("Other.\n\nText"));
  });
});
