import { describe, expect, test } from "vitest";
import { titleOf } from "./title";

describe("a note's title is its first real level-one heading", () => {
  test("the plain case, and the fallback when there is none", () => {
    expect(titleOf("# Plan\n\nbody", "Untitled")).toBe("Plan");
    expect(titleOf("no heading here\n## Sub", "Untitled")).toBe("Untitled");
    expect(titleOf("", "Old title")).toBe("Old title");
  });
  test("a YAML comment in the frontmatter is not the title", () => {
    expect(titleOf("---\n# not a title\ntags: [a]\n---\n\n# Real\n", "x")).toBe("Real");
    expect(titleOf("---\n# only a yaml comment\n---\nbody", "x")).toBe("x");
  });
  test("a `---` that never closes is text, not frontmatter", () => {
    expect(titleOf("---\n# Heading under a rule\n", "x")).toBe("Heading under a rule");
  });
  test("a heading-looking line in a fenced block is skipped (backticks or tildes, longer closers)", () => {
    expect(titleOf("```sh\n# a shell comment\n```\n\n# Real", "x")).toBe("Real");
    expect(titleOf("~~~\n# inside\n~~~\n# Real", "x")).toBe("Real");
    expect(titleOf("````\n```\n# still inside\n```\n````\n# Real", "x")).toBe("Real");
    expect(titleOf("```\n# never closed", "x")).toBe("x");
  });
  test("CRLF line breaks, a closing run of hashes, and a hashtag", () => {
    expect(titleOf("---\r\n# c\r\n---\r\n# Real ##\r\n", "x")).toBe("Real");
    expect(titleOf("#tag\n# Real", "x")).toBe("Real");
    expect(titleOf("## Two\n# One", "x")).toBe("One");
  });
});
