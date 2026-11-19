import { describe, it } from "vitest";
import { execFileSync } from "node:child_process";
import { join } from "node:path";

describe("the docs gate", () => {
  it("every exported item of the package has a doc comment", () => { execFileSync("node", [join(__dirname, "../scripts/check-docs.mjs")], { stdio: "pipe" }); });
});
