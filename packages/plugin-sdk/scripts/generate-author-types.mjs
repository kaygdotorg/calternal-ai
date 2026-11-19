// Generate the authoring types from the same schemas the build validates.
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { compile } from "json-schema-to-typescript";

const contract = new URL("../../../contract/", import.meta.url);
const names = [
  "plugin-manifest-v2.schema.json",
  "plugin-manifest-v1.schema.json",
  "description-v2.schema.json",
  "host-catalogue-v2.json",
];
const documents = new Map(await Promise.all(names.map(async name => [
  name, JSON.parse(await readFile(new URL(name, contract), "utf8")),
])));
const [manifest, , description, catalogue] = names;
const definitions = {};
const references = new Map();

function reference(document, pointer, preferredName) {
  const key = document + pointer;
  if (!references.has(key)) {
    const leaf = preferredName ?? pointer.split("/").at(-1);
    const name = Object.hasOwn(definitions, leaf)
      ? document.split(".")[0].replaceAll("-", "_") + "_" + leaf
      : leaf;
    definitions[name] = {};
    references.set(key, name);
    const target = pointer.slice(2).split("/").filter(Boolean).reduce(
      (value, part) => value[part.replaceAll("~1", "/").replaceAll("~0", "~")],
      documents.get(document),
    );
    definitions[name] = { ...structural(target, document), title: name };
  }
  return { $ref: "#/definitions/" + references.get(key) };
}

function structural(schema, document) {
  if (typeof schema === "boolean") return schema;
  if (schema.$ref) {
    const [file, fragment = ""] = schema.$ref.split("#");
    const target = file ? file.split("/").at(-1) : document;
    return reference(target, "#" + fragment);
  }

  const output = { ...schema };
  // Conditional and value constraints are checked by the build validators.
  for (const key of ["$id", "$schema", "$defs", "definitions", "if", "then", "else", "not", "dependentRequired"]) {
    delete output[key];
  }
  for (const key of ["properties", "patternProperties"]) {
    if (output[key]) output[key] = Object.fromEntries(
      Object.entries(output[key]).map(([name, value]) => [name, structural(value, document)]),
    );
  }
  if (output.allOf) {
    output.allOf = output.allOf.filter(clause => !("if" in clause));
    if (output.allOf.length === 0) delete output.allOf;
  }
  for (const key of ["oneOf", "anyOf", "allOf"]) {
    if (output[key]) output[key] = output[key].map(value => structural(value, document));
  }
  for (const key of ["additionalProperties", "items"]) {
    if (typeof output[key] === "object") output[key] = structural(output[key], document);
  }
  if (output.type === "object" && !output.properties && output.additionalProperties === undefined) {
    output.additionalProperties = true;
  }
  return output;
}

const properties = {
  manifest: reference(manifest, "#", "ManifestV2"),
  description: reference(description, "#/$defs/AppDescription"),
};
const windowSize = documents.get(description).$defs.Bind.properties.window.properties.size.default;
if (windowSize !== 60 || windowSize !== documents.get(catalogue).query.window.default_rows) {
  throw new Error("The SDK, description and host catalogue must agree on the default Bind window.");
}
const generated = new URL("../generated/", import.meta.url);
await mkdir(generated, { recursive: true });
await writeFile(new URL("author-types.d.ts", generated), await compile({
  type: "object",
  properties,
  definitions,
}, "AuthorContracts", {
  additionalProperties: false,
  maxItems: -1,
  bannerComment: "/* Generated from the sealed plugin API schemas. */",
}));
