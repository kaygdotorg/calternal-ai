import { spawnSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { build } from "esbuild";
import { validateManifestV1 } from "@calternal/plugin-sdk";

const manifest = validateManifestV1(JSON.parse(await readFile("manifest.json", "utf8")));
await writeFile("manifest.generated.ts", `export default ${JSON.stringify(manifest, null, 2)} as const;\n`);
if (!process.argv.includes("--draft")) {
  const tsc = spawnSync(process.execPath, ["node_modules/typescript/bin/tsc", "--noEmit"], { stdio: "inherit" });
  if (tsc.error) {
    process.stderr.write(`Could not run TypeScript: ${tsc.error.message}\n`);
    process.exit(1);
  }
  if (tsc.signal) {
    process.stderr.write(`TypeScript exited on signal ${tsc.signal}\n`);
    process.exit(1);
  }
  if (tsc.status !== 0) process.exit(tsc.status ?? 1);
}
await build({ entryPoints: ["worker.ts"], outfile: "dist/worker.js", bundle: true,
  platform: "node", format: "cjs", logLevel: "error" }).catch(() => { process.exitCode = 1; });
