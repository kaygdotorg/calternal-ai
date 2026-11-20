// Generate the SDK author types and copy the validation inputs into the SDK.
import { copyFile, mkdir, writeFile } from "node:fs/promises";
import { generateAuthorTypes } from "../../contracts/scripts/schema-to-types.mjs";

import { contractPin } from "./contract-pin.mjs";

const contract = await contractPin(new URL("../../../contract/", import.meta.url));
const generated = new URL("../generated/", import.meta.url);
const packagedContract = new URL("contract/", generated);
const { text, windowSize, contributionFrameBytes } = await generateAuthorTypes(contract);
if (windowSize !== 60) throw new Error("The SDK must agree with the contract's default Bind window.");

await mkdir(packagedContract, { recursive: true });
await Promise.all([
  writeFile(new URL("author-types.d.ts", generated), text),
  writeFile(new URL("contribution-limit.mjs", generated),
    `export const contributionFrameBytes = ${JSON.stringify(contributionFrameBytes)};\n`),
  ...[
    "verify-description.py",
    "description_verification.py",
    "description_input.py",
    "description_events.py",
    "description_confirmation.py",
    "description_intent_trace.py",
    "description_title.py",
    "json-schema-draft-07.json",
    "description-v2.schema.json",
    "host-catalogue-v2.json",
    "plugin-manifest-v1.schema.json",
    "plugin-manifest-v2.schema.json",
  ].map(file => copyFile(new URL(file, contract), new URL(file, packagedContract))),
]);
