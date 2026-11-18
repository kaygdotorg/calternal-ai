import { execFileSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { build } from "esbuild";
import { validateManifestV1 } from "@calternal/plugin-sdk";

const manifest = validateManifestV1(JSON.parse(await readFile("manifest.json", "utf8")));
await writeFile("manifest.generated.ts", `export default ${JSON.stringify(manifest, null, 2)} as const;\n`);
execFileSync(process.execPath, ["node_modules/typescript/bin/tsc", "--noEmit"], { stdio: "inherit" });
await build({ entryPoints: ["worker.ts"], outfile: "dist/worker.js", bundle: true,
  platform: "node", format: "cjs", logLevel: "error" });
