// The standalone host's serde declarations are the source of its wire surface.
import { readFile, writeFile } from "node:fs/promises";

const source = relative => readFile(new URL(`../../../crates/${relative}`, import.meta.url), "utf8");
const [write, query, wire] = await Promise.all([
  source("host/src/write.rs"),
  source("host/src/query.rs"),
  source("plugin-host/src/operations/wire.rs"),
]);

function body(source, signature) {
  const start = source.indexOf(`${signature} {\n`);
  const end = source.indexOf("\n}", start);
  if (start < 0 || end < 0) throw Error(`Missing Rust declaration ${signature}`);
  return source.slice(start + signature.length + 3, end);
}

const fieldTypes = new Map([
  ["Fields", "Fields"], ["String", "string"], ["Uuid", "ItemId"],
  ["records::Input", "RecordBody"], ["u64", "number"], ["usize", "number"],
  ["Vec<String>", "readonly string[]"],
]);
const property = (name, rustType, attrs) => {
  const option = /^Option<(.+)>$/.exec(rustType);
  const type = fieldTypes.get(option?.[1] ?? rustType);
  if (!type) throw Error(`Unsupported host wire type ${rustType}`);
  const rename = attrs.match(/rename = "([^"]+)"/)?.[1] ?? name;
  return `${JSON.stringify(rename)}${option || attrs.includes("default") ? "?" : ""}: ${type}`;
};
function fields(lines, spaces) {
  const declarations = [];
  let attrs = "";
  for (const line of lines.split("\n")) {
    if (!line.trim() || line.trimStart().startsWith("//")) continue;
    if (line.trimStart().startsWith("#[serde(")) {
      attrs += line;
      continue;
    }
    const match = new RegExp(`^ {${spaces}}(\\w+): (.+),$`).exec(line);
    if (!match) throw Error(`Unsupported host field declaration ${line}`);
    declarations.push([match[1], property(match[1], match[2], attrs)]);
    attrs = "";
  }
  return new Map(declarations);
}
const variants = [];
let variantAttrs = "";
let active;
for (const line of body(write, "enum Change").split("\n")) {
  if (!line.trim() || line.trimStart().startsWith("//")) continue;
  if (line.startsWith("    #[serde(")) { variantAttrs += line; continue; }
  const name = /^    (\w+) \{$/.exec(line)?.[1];
  if (name) {
    if (active) throw Error(`Unexpected change declaration ${line}`);
    active = { name: variantAttrs.match(/rename = "([^"]+)"/)?.[1] ?? name.toLowerCase(), lines: [] };
    variantAttrs = "";
  } else if (line === "    },") {
    variants.push([active.name, [...fields(active.lines.join("\n"), 8).values()].join("; ")]);
    active = undefined;
  } else if (active) {
    active.lines.push(line);
  } else {
    throw Error(`Unsupported host change declaration ${line}`);
  }
}

const methods = [];
let wireName;
for (const line of body(wire, "pub enum Method").split("\n")) {
  if (!line.trim() || line.trimStart().startsWith("//")) continue;
  const rename = /^    #\[serde\(rename = "([^"]+)"\)\]$/.exec(line);
  if (rename) { wireName = rename[1]; continue; }
  const variant = /^    (\w+),$/.exec(line)?.[1];
  if (!variant || !wireName) throw Error(`Unsupported callback method declaration ${line}`);
  methods.push(`  ${variant[0].toLowerCase()}${variant.slice(1)}: ${JSON.stringify(wireName)},`);
  wireName = undefined;
}

const queryFields = fields(body(query, "pub(crate) struct Query"), 4);
const queryShape = (required, omitted) => [...queryFields]
  .filter(([name]) => !omitted.includes(name))
  .map(([name, declaration]) => name === required ? declaration.replace("?:", ":") : declaration)
  .join("; ");

const generated = `// Generated from host serde declarations by generate-operation-types.mjs.
import type { Fields, ItemId, RecordBody } from "./operation-api.js";

/** A kind query or a stored collection query accepted by the standalone host. */
export type Query =
  | { ${queryShape("from", ["collection"])} }
  | { ${queryShape("collection", ["from", "filter", "sort"])} };

/** Changes supported by the host's atomic write route. */
export type Change =
${variants.map(([name, args]) => `  | { ${JSON.stringify(name)}: { ${args} } }`).join("\n")};

/** Change discriminants accepted by the host. */
export const operationChanges = ${JSON.stringify(variants.map(([name]) => name))} as const;

/** Callback wire names accepted by the host. */
export const operationMethods = {
${methods.join("\n")}
} as const;

/** One callback wire name. */
export type OperationMethod = typeof operationMethods[keyof typeof operationMethods];
`;
await writeFile(new URL("../src/operation-contract.generated.ts", import.meta.url), generated);
