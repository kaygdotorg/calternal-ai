import { execFileSync } from "node:child_process";
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

const pin = "35a5095980947ada8adb5b5431150d7490ab8691";
const here = new URL("./", import.meta.url);
const root = new URL("../../", here);
const scratch = new URL(".build/", here);
const generated = new URL("generated/", here);
const contract = new URL("contract/", scratch);
await mkdir(scratch, { recursive: true });
await mkdir(generated, { recursive: true });
// Keep draft artifacts outside the sealed SDK tree and out of the package.
execFileSync("git", ["archive", "-o", fileURLToPath(new URL("contract.tar", scratch)), pin, "contract", "packages/plugin-sdk/scripts", "packages/contracts/scripts/schema-to-types.mjs"], { cwd: root });
execFileSync("tar", ["-xf", "contract.tar"], { cwd: scratch });
const sdk = new URL("packages/plugin-sdk/scripts/", scratch);
const sdkGenerated = new URL("../generated/", sdk);
const env = { ...process.env, CALTERNAL_PLUGIN_CONTRACT: fileURLToPath(contract), CALTERNAL_PLUGIN_CONTRACT_REVISION: "14.40" };
execFileSync(process.execPath, [fileURLToPath(new URL("generate-author-types.mjs", sdk))], { env, stdio: "inherit" });
await copyFile(new URL("author-types.d.ts", sdkGenerated), new URL("author.d.ts", generated));
await copyFile(new URL("contribution-limit.mjs", sdkGenerated), new URL("contribution-limit.mjs", generated));
for (const module of ["author-context", "contribution-frame"]) {
  const source = await readFile(new URL(`${module}.mjs`, sdk), "utf8");
  await writeFile(new URL(`${module}.mjs`, generated), source.replace("../generated/contribution-limit.mjs", "./contribution-limit.mjs"));
  const types = await readFile(new URL(`${module}.d.mts`, sdk), "utf8");
  await writeFile(new URL(`${module}.d.mts`, generated), types.replace("../generated/author-types.js", "./author.js"));
}
const { generateDescriptionTypes } = await import(new URL("packages/contracts/scripts/schema-to-types.mjs", scratch).href);
await writeFile(new URL("wire.d.ts", generated), await generateDescriptionTypes(contract));
execFileSync(fileURLToPath(new URL("node_modules/.bin/tsc", root)), ["-p", fileURLToPath(new URL("tsconfig.json", here))], { stdio: "inherit" });

const packages = [here, new URL("../calternal-threads-claude-code/", here)];
for (const [index, plugin] of packages.entries()) {
  const bundled = await build({
    entryPoints: [fileURLToPath(new URL("src/worker.ts", plugin))],
    bundle: true, platform: "node", format: "esm", write: false,
  });
  const worker = new URL(`worker-${index}.mjs`, scratch);
  await writeFile(worker, bundled.outputFiles[0].contents);
  execFileSync(process.execPath, [
    fileURLToPath(new URL("build-author-package.mjs", sdk)),
    fileURLToPath(new URL("src/plugin.ts", plugin)), fileURLToPath(new URL("dist/", plugin)), fileURLToPath(worker),
  ], { env, stdio: "inherit" });
}
const outputs = packages.map(plugin => fileURLToPath(new URL("dist/", plugin)));
execFileSync("python3", [
  "-B", fileURLToPath(new URL("check-shapes.py", here)), fileURLToPath(contract), ...outputs,
], { stdio: "inherit" });
if (process.argv.includes("--conformance")) {
  const verifier = fileURLToPath(new URL("verify-description.py", contract));
  execFileSync("python3", ["-B", verifier, "--version"], { stdio: "inherit" });
  for (const output of outputs) {
    try {
      execFileSync("python3", ["-B", verifier, "--package", output], { stdio: "inherit" });
    } catch {
      process.exitCode = 1;
    }
  }
}
