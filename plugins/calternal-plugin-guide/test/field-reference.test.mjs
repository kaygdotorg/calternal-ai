import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const root = new URL("../../../", import.meta.url);
const read = path => readFileSync(new URL(path, root), "utf8");
const v1 = JSON.parse(read("contract/plugin-manifest-v1.schema.json")).definitions;
const v2 = JSON.parse(read("contract/plugin-manifest-v2.schema.json")).definitions;
const description = JSON.parse(read("contract/description-v2.schema.json")).$defs;
const walkthrough = read("plugins/calternal-plugin-guide/walkthrough.md");

test("the guide lists every allowed operation value and form input from the schemas", () => {
  const columns = {
    risk: v1.OperationRisk.oneOf.flatMap(entry => entry.enum),
    idempotency: v1.ManifestIdempotency.oneOf.flatMap(entry => entry.enum),
    methods: v2.hostMethod.enum,
    required_capability: v1.ManifestCapability.oneOf.flatMap(entry => entry.enum),
    "FormField.input": description.FormField.properties.input.enum,
  };
  for (const [field, expected] of Object.entries(columns)) {
    const row = walkthrough.split("\n").find(line => line.startsWith(`| \`${field}\` | `));
    assert.ok(row, `Missing row for ${field}`);
    assert.deepEqual([...row.split("|")[2].matchAll(/\`([^\`]+)\`/g)].map(match => match[1]), expected, field);
  }
});
