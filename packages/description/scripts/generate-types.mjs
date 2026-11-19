// Generates src/generated/model.ts from the two sealed schemas (contract/description-v2.schema.json and contract/host-catalogue-v2.json) so no shape the contract owns is written twice. The merge of the schemas and the
// compiler are the shared generator of @calternal/contracts (one place, also used by the plugin SDK). `--check` regenerates in memory and fails when the committed file differs (the test and `npm run check` run it).
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { generateDescriptionTypes } from "../../contracts/scripts/schema-to-types.mjs";

export const generate = () => generateDescriptionTypes(new URL("../../../contract/", import.meta.url));

const out = join(dirname(fileURLToPath(import.meta.url)), "../src/generated/model.ts");
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const text = await generate();
  if (process.argv.includes("--check")) {
    let have = ""; try { have = readFileSync(out, "utf8"); } catch { /* none */ }
    if (have !== text) { console.error("src/generated/model.ts is out of date with the contract schemas: run `npm run types -w @calternal/description`."); process.exit(1); }
  } else writeFileSync(out, text);
}
