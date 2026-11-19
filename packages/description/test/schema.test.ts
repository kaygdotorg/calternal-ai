import { describe, expect, it } from "vitest";
import { execFileSync } from "node:child_process";
import { join } from "node:path";
import { loadFixtures, makeValidators } from "./fixtures";
import { refusalOf } from "../src";

describe("the model is the contract", () => {
  it("src/generated/model.ts is current with the sealed schemas", () => {
    execFileSync("node", [join(__dirname, "../scripts/generate-types.mjs"), "--check"], { stdio: "pipe" });
  });

  const v = makeValidators();
  const fixtures = loadFixtures();

  it("finds the fixtures", () => { expect(fixtures.filter((f) => !f.invalid).length).toBeGreaterThan(20); });

  for (const f of fixtures.filter((f) => !f.invalid)) {
    it(`valid fixture ${f.name} passes its schema entry point`, () => {
      const ok = v[f.kind](f.doc);
      expect(ok, JSON.stringify(v[f.kind].errors?.slice(0, 2))).toBe(true);
    });
  }

  it("the invalid fixtures the schema can reject are rejected (the rest are the verifier's semantic rules)", () => {
    const rejected = fixtures.filter((f) => f.invalid).filter((f) => !v[f.kind](f.doc)).length;
    expect(rejected).toBeGreaterThan(10);
  });

  it("every valid description is one the renderer accepts to draw", () => {
    for (const f of fixtures.filter((f) => !f.invalid && f.kind === "description" && (f.doc as { kind?: string }).kind === "app")) expect(refusalOf(f.doc), f.name).toBeNull();
  });

  it("refuses what it cannot draw, whole", () => {
    expect(refusalOf({ version: 3, kind: "app", revision: 1 })?.reason).toMatch(/version 3/);
    expect(refusalOf({ version: 2, kind: "app", revision: 1, requires: ["time-travel"] })?.reason).toMatch(/time-travel/);
    expect(refusalOf({ version: 2, kind: "view", revision: 1 })?.reason).toMatch(/kind view/);
  });
});
