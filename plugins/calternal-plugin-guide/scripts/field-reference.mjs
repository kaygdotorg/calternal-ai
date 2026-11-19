import { readFile } from "node:fs/promises";

const contract = new URL("../../../contract/", import.meta.url);
const manifest = "plugin-manifest-v2.schema.json";
const description = "description-v2.schema.json";

/** Render the author field choices from the offline schema registry. */
export async function fieldReference() {
  const documents = new Map(await Promise.all([
    manifest, "plugin-manifest-v1.schema.json", description,
  ].map(async name => [name, JSON.parse(await readFile(new URL(name, contract), "utf8"))])));

  function values(schema, file) {
    if (schema.enum) return schema.enum;
    if (schema.$ref) {
      const [name, pointer] = schema.$ref.split("#/");
      const target = name || file;
      const definition = pointer.split("/").reduce((node, key) => node[key], documents.get(target));
      return values(definition, target);
    }
    const branches = schema.oneOf ?? schema.allOf;
    if (branches) return branches.flatMap(branch => values(branch, file));
    if (schema.items) return values(schema.items, file);
    throw new Error(`Expected a closed enum in ${file}.`);
  }

  const operation = documents.get(manifest).definitions.operation.properties;
  const formInput = documents.get(description).$defs.FormField.properties.input;
  const rows = [
    ...["risk", "idempotency", "methods", "required_capability"].map(field => [
      field, values(operation[field], manifest),
    ]),
    ["FormField.input", values(formInput, description)],
  ];
  return [
    "| Field | Allowed values |",
    "| --- | --- |",
    ...rows.map(([field, allowed]) => `| \`${field}\` | ${allowed.map(value => `\`${value}\``).join(", ")} |`),
  ].join("\n");
}
