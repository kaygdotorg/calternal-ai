// Generate the SDK author types, broker method table and author limits, and
// copy the validation inputs into the SDK. Every generated value is read from
// the pinned contract, so the SDK states no limit of its own.
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { brokerMethods, generateAuthorTypes } from "../../contracts/scripts/schema-to-types.mjs";

import { contractPin } from "./contract-pin.mjs";
import { authorLimits } from "./limits.mjs";

const contract = await contractPin(new URL("../../../contract/", import.meta.url));
const generated = new URL("../generated/", import.meta.url);
const sourceGenerated = new URL("../src/generated/", import.meta.url);
const packagedContract = new URL("contract/", generated);
const { text, windowSize } = await generateAuthorTypes(contract);
if (windowSize !== 60) throw new Error("The SDK must agree with the contract's default Bind window.");

const read = async name => JSON.parse(await readFile(new URL(name, contract), "utf8"));
const [catalogue, description, manifest] = await Promise.all(
  ["host-catalogue-v2.json", "description-v2.schema.json", "plugin-manifest-v2.schema.json"].map(read));
const methods = brokerMethods(catalogue);
// Catalogue object definitions whose properties include ItemRefs, so an
// operation whose input is such a definition can still infer its target.
const isItemRef = schema => schema?.$ref === "#/$defs/ItemRef";
const itemRefFields = Object.fromEntries(Object.entries(catalogue.$defs)
  .map(([name, schema]) => [name, Object.entries(schema.properties ?? {}).filter(([, value]) => isItemRef(value)).map(([field]) => field)])
  .filter(([, fields]) => fields.length));
const limits = authorLimits(catalogue, description, manifest);

const header = "/* Generated from the pinned plugin API contract by scripts/generate-author-types.mjs. */\n";
const methodsModule = `${header}
/** Contract revision these tables were generated from. */
export const CONTRACT_REVISION = ${JSON.stringify(catalogue.exchanges_revision)};

/** Catalogue definition pointers of one broker exchange. */
export interface BrokerExchange {
  /** Pointer to the input definition, such as \`#/$defs/ItemCreate\`. */
  readonly input: string;
  /** Pointer to the reply definition. */
  readonly output: string;
}

/**
 * Catalogue input and output definition pointers for every broker method with
 * a pinned exchange. The fake host validates each call against these.
 */
export const BROKER_METHODS: Readonly<Record<string, BrokerExchange>> = ${JSON.stringify(methods, null, 2)};

/** ItemRef-typed properties of each catalogue object definition that has any, for target inference. */
export const CATALOGUE_ITEM_FIELDS: Readonly<Record<string, readonly string[]>> = ${JSON.stringify(itemRefFields)};
`;
const limitsModule = `${header}
/** Every numeric limit an author can hit, each read from one contract pointer. */
export interface Limits {
${limits.map(({ name, value, doc, source }) => `  /** ${doc} Source: \`${source}\`. */\n  readonly ${name}: ${value};`).join("\n")}
}

/** The name of one limit. */
export type LimitName = keyof Limits;

/** Where one limit comes from, for diagnostics. */
export interface LimitSource {
  /** The bound. */
  readonly value: number;
  /** What the bound counts: bytes, characters, fields and so on. */
  readonly unit: string;
  /** The contract document and JSON pointer it is read from. */
  readonly source: string;
}

/** The limits as constants, for example \`LIMITS.ITEM_APPEND_BYTES\`. */
export const LIMITS: Limits = {
${limits.map(({ name, value }) => `  ${name}: ${value},`).join("\n")}
};

/** Every limit with its unit and contract source, for preflight diagnostics. */
export const LIMIT_SOURCES: Readonly<Record<LimitName, LimitSource>> = {
${limits.map(({ name, value, source, unit }) => `  ${name}: { value: ${value}, unit: ${JSON.stringify(unit)}, source: ${JSON.stringify(source)} },`).join("\n")}
};
`;

await mkdir(packagedContract, { recursive: true });
await mkdir(sourceGenerated, { recursive: true });
await Promise.all([
  writeFile(new URL("author-types.d.ts", generated), text),
  writeFile(new URL("broker-methods.ts", sourceGenerated), methodsModule),
  writeFile(new URL("limits.ts", sourceGenerated), limitsModule),
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
