import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, expect, it } from "vitest";
import { verifyPackage } from "../scripts/verify-package.mjs";

const cli = fileURLToPath(new URL("../scripts/build-author-package.mjs", import.meta.url));
const sdk = new URL("../dist/index.js", import.meta.url).href;
const scratch: string[] = [];

afterEach(() => {
  for (const directory of scratch.splice(0)) rmSync(directory, { recursive: true, force: true });
});

function setup() {
  const directory = mkdtempSync(join(tmpdir(), "calternal-author-test-"));
  scratch.push(directory);
  const entry = join(directory, "entry.mjs");
  const output = join(directory, "output");
  const manifestPath = join(output, "manifest.json");
  const run = (source: string) => {
    writeFileSync(entry, `import { definePlugin, defineApp, defineView, defineKind } from ${JSON.stringify(sdk)};\n${source}`);
    return spawnSync(process.execPath, [cli, entry, output], { encoding: "utf8" });
  };
  return { output, manifestPath, run };
}

const firstList = `
  const kind = defineKind({
    id: "link", icon: "book", storage: { hidden: true }, claimed_text: true,
    schema: { type: "object", properties: { title: { type: "string" } } },
  });
  const app = defineApp({
    id: "links", name: "Links", icon: "book",
    view: defineView({
      binds: [{ id: "items", source: { type: "list", kind: "link" } }],
      root: { type: "list", bind: "items" },
    }),
  });
  export default definePlugin({
    api_version: 2, id: "links", name: "Links", version: "1.0.0",
    icon: "book", kinds: [kind], apps: [app],
  });
`;

it("verifies the emitted template with the generated manifest and pins its bytes", () => {
  const { output, manifestPath, run } = setup();
  const result = run(firstList);
  expect(result.stderr).toBe("");
  expect(result.status).toBe(0);
  expect(result.stdout).toMatch(/ok\s+app-0\.json/);

  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  expect(manifest).toMatchObject({ api_version: 2, reads: ["link"], network: "none", operations: [] });
  const { path, sha256 } = manifest.apps[0].description.template;
  const bytes = readFileSync(join(output, path));
  expect(createHash("sha256").update(bytes).digest("hex")).toBe(sha256);
  expect(JSON.parse(bytes.toString()).binds[0].window).toEqual({ size: 60, count: "exact" });
});

it("reports manifest schema failures and refuses a Bind that the manifest never declared", () => {
  const { manifestPath, run } = setup();
  expect(run(firstList).status).toBe(0);
  const previous = readFileSync(manifestPath, "utf8");

  const invalidManifest = run(firstList.replace("api_version: 2", "api_version: 1"));
  expect(invalidManifest.status).toBe(1);
  expect(invalidManifest.stderr).toMatch(/manifest\.json.*api_version/);

  const invalidBind = run(firstList.replace('type: "list", kind: "link"', 'type: "list", kind: "missing"'));
  expect(invalidBind.status).toBe(1);
  expect(invalidBind.stdout).toMatch(/missing.*not in.*reads/);
  expect(readFileSync(manifestPath, "utf8")).toBe(previous);
});

it("rejects an emitted template whose bytes no longer match its pin", async () => {
  const { output, manifestPath, run } = setup();
  expect(run(firstList).status).toBe(0);
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const asset = join(output, manifest.apps[0].description.template.path);
  writeFileSync(asset, readFileSync(asset, "utf8") + " ");
  await expect(verifyPackage(output)).rejects.toThrow(/SHA-256 pin/);
});
