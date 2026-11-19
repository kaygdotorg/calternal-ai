// Generate the SDK author types and copy the validation inputs into the SDK.
import { copyFile, mkdir, writeFile } from "node:fs/promises";
import { generateAuthorTypes } from "../../contracts/scripts/schema-to-types.mjs";

const contract = new URL("../../../contract/", import.meta.url);
const generated = new URL("../generated/", import.meta.url);
const packagedContract = new URL("contract/", generated);
const { text, windowSize } = await generateAuthorTypes(contract);
if (windowSize !== 60) throw new Error("The SDK must agree with the contract's default Bind window.");

await mkdir(packagedContract, { recursive: true });
await Promise.all([
  writeFile(new URL("author-types.d.ts", generated), text),
  ...[
    "verify-description.py",
    "description-v2.schema.json",
    "host-catalogue-v2.json",
    "plugin-manifest-v1.schema.json",
    "plugin-manifest-v2.schema.json",
  ].map(file => copyFile(new URL(file, contract), new URL(file, packagedContract))),
]);
