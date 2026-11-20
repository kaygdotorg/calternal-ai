import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { copyFile, mkdir, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";
import { generateAuthorTypes, generateDescriptionTypes } from "../../packages/contracts/scripts/schema-to-types.mjs";

const pin = "b89586dd306784e94ca09992ae05180d7bb7a686";
const here = new URL("./", import.meta.url);
const root = new URL("../../", here);
const scratch = new URL(".build/", here);
const generated = new URL("generated/", here);
const contract = new URL("contract/", scratch);
await mkdir(scratch, { recursive: true });
await mkdir(generated, { recursive: true });
// Keep draft artifacts outside the sealed SDK tree and out of the package.
execFileSync("git", ["archive", "-o", fileURLToPath(new URL("contract.tar", scratch)), pin, "contract"], { cwd: root });
execFileSync("tar", ["-xf", "contract.tar"], { cwd: scratch });
const author = await generateAuthorTypes(contract);
await writeFile(new URL("author.d.ts", generated), author.text);
await writeFile(new URL("wire.d.ts", generated), await generateDescriptionTypes(contract));
execFileSync(fileURLToPath(new URL("node_modules/.bin/tsc", root)), ["-p", fileURLToPath(new URL("tsconfig.json", here))], { stdio: "inherit" });

const digest = bytes => createHash("sha256").update(bytes).digest("hex");
const json = value => Buffer.from(JSON.stringify(value, null, 2) + "\n");

const packages = [here, new URL("../calternal-threads-claude-code/", here)];
for (const plugin of packages) {
  const source = new URL("src/plugin.ts", plugin);
  const { default: authored } = await import(source.href);
  const output = new URL("dist/", plugin);
  await mkdir(output, { recursive: true });
  // Mirror the sealed SDK package layout without depending on its sealed
  // verifier to publish draft artifacts. Run the draft tool independently.
  async function asset(prefix, extension, bytes) {
    const sha256 = digest(bytes);
    const path = `${prefix}-${sha256}.${extension}`;
    await writeFile(new URL(path, output), bytes);
    return { path, sha256 };
  }
  const apps = await Promise.all(authored.apps.map(async ({ metadata, template }, index) => ({
    ...metadata, description: { id: "app", template: await asset(`app-${index}`, "json", json(template)) },
  })));
  const bundled = await build({
    entryPoints: [fileURLToPath(new URL("src/worker.ts", plugin))],
    bundle: true, platform: "node", format: "esm", write: false,
  });
  const worker = await asset("worker", "mjs", bundled.outputFiles[0].contents);
  for (const contribution of authored.metadata.contributions ?? []) {
    const path = contribution.harness.entry.path;
    await copyFile(new URL(path, source), new URL(path, output));
  }
  await writeFile(new URL("manifest.json", output), json({ ...authored.metadata, apps, worker }));
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
