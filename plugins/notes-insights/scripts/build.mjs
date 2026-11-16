import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { build } from "esbuild";

execFileSync("tsc", ["--noEmit", "-p", "tsconfig.json"], { stdio: "inherit" });
await mkdir("dist", { recursive: true });

const result = await build({
  entryPoints: ["src/worker.ts"],
  outfile: "dist/worker.js",
  bundle: true,
  platform: "node",
  format: "esm",
  target: "node22",
  minify: true,
  metafile: true,
});

const output = result.metafile.outputs["dist/worker.js"];
if (!output || output.imports.length !== 0) {
  throw new Error("worker bundle must have no external imports");
}

const bundle = await readFile("dist/worker.js");
const digest = createHash("sha256").update(bundle).digest("hex");
await writeFile("dist/worker.js.sha256", `${digest}\n`, "utf8");
console.log(`notes-insights worker sha256: ${digest}`);
