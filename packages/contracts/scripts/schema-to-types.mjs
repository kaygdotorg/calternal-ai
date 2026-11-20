import { readFile } from "node:fs/promises";
import { compile } from "json-schema-to-typescript";

const description = "description-v2.schema.json";
const catalogue = "host-catalogue-v2.json";
const manifest = "plugin-manifest-v2.schema.json";

async function load(directory, names) {
  return new Map(await Promise.all(names.map(async name => [
    name, JSON.parse(await readFile(new URL(name, directory), "utf8")),
  ])));
}

function bundle(documents) {
  const definitions = {};
  const references = new Map();

  function reference(document, pointer, preferredName) {
    const key = document + pointer;
    if (!references.has(key)) {
      const leaf = preferredName ?? pointer.split("/").at(-1);
      const catalogueOwnsName = document !== catalogue && Object.hasOwn(documents.get(catalogue).$defs, leaf);
      const name = Object.hasOwn(definitions, leaf) || catalogueOwnsName
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
    for (const key of ["properties", "patternProperties", "dependentSchemas"]) {
      if (output[key]) output[key] = Object.fromEntries(
        Object.entries(output[key]).map(([name, value]) => [name, structural(value, document)]),
      );
    }
    if (output.allOf) {
      output.allOf = output.allOf.filter(clause => typeof clause === "boolean" || !("if" in clause));
      if (output.allOf.length === 0) delete output.allOf;
    }
    // json-schema-to-typescript consumes Draft 7 tuple syntax.
    if (output.prefixItems) {
      output.additionalItems = output.items ?? true;
      output.items = output.prefixItems;
      delete output.prefixItems;
    }
    for (const key of ["oneOf", "anyOf", "allOf"]) {
      if (output[key]) output[key] = output[key].map(value => structural(value, document));
    }
    for (const key of ["additionalProperties", "propertyNames", "items", "contains", "additionalItems",
      "unevaluatedProperties", "unevaluatedItems"]) {
      if (Array.isArray(output[key])) output[key] = output[key].map(value => structural(value, document));
      else if (typeof output[key] === "object") output[key] = structural(output[key], document);
    }
    if (output.type === "object" && !output.properties && output.additionalProperties === undefined) {
      output.additionalProperties = true;
    }
    return output;
  }
  return { definitions, reference };
}

const options = {
  additionalProperties: false,
  maxItems: -1,
  bannerComment: "/* Generated from the pinned plugin API schemas. */",
};

/**
 * Compile the manifest and authored app description against the contract
 * registry. The SDK checks the remaining value constraints during its build.
 */
export async function generateAuthorTypes(directory) {
  const documents = await load(directory, [manifest, "plugin-manifest-v1.schema.json", description, catalogue]);
  const { definitions, reference } = bundle(documents);
  const properties = {
    manifest: reference(manifest, "#", "ManifestV2"),
    description: reference(description, "#/$defs/AppDescription"),
  };
  const host = documents.get(catalogue);
  let context = "";
  if (host.$defs.AuthorInvocationContext) {
    const methods = {
      ...host.item_engine,
      ...host.execution_engines,
      ...host.extension_points.calls,
      start_job: host.execution_intents.start_job,
      alarm_set: host.alarms.set,
      alarm_cancel: host.alarms.cancel,
      item_append: host.file_append,
      item_view_context: host.view_snapshots.context,
      item_view_publish: host.view_snapshots.publish,
    };
    const methodSchemas = Object.fromEntries(Object.entries(methods)
      .filter(([, method]) => method?.input?.$ref && method?.output?.$ref)
      .map(([name, method]) => [name, {
        type: "object", required: ["input", "output"], properties: {
          input: reference(catalogue, method.input.$ref),
          output: reference(catalogue, method.output.$ref),
        },
      }]));
    properties.broker = { type: "object", properties: methodSchemas, required: Object.keys(methodSchemas) };
    properties.author = reference(catalogue, "#/$defs/AuthorInvocationContext");
    properties.job = reference(catalogue, "#/$defs/ExecutionJobContext");
    properties.alarm = reference(catalogue, "#/$defs/AlarmDelivery");
    context = `
/** Stable handler data from an authenticated invocation and its typed host broker. */
export interface RevisionedAuthorContext {
  author: NonNullable<AuthorContracts["author"]>;
  job?: NonNullable<AuthorContracts["job"]>;
  alarm?: NonNullable<AuthorContracts["alarm"]>;
  call<K extends keyof NonNullable<AuthorContracts["broker"]>>(
    method: K, input: NonNullable<AuthorContracts["broker"]>[K]["input"],
  ): Promise<NonNullable<AuthorContracts["broker"]>[K]["output"]>;
}
`;
  }
  const text = await compile({ type: "object", properties, definitions }, "AuthorContracts", options) + context;
  const windowSize = documents.get(description).$defs.Bind.properties.window.properties.size.default;
  if (windowSize !== documents.get(catalogue).query.window.default_rows) {
    throw new Error("The description and host catalogue must agree on the default Bind window.");
  }
  return { text, windowSize };
}

/**
 * Compile the full description and wire model for a renderer. Catalogue
 * names are reserved first, so the host definitions own the shared names.
 */
export async function generateDescriptionTypes(directory) {
  const documents = await load(directory, [catalogue, description]);
  const { definitions, reference } = bundle(documents);
  for (const [file, schema] of documents) {
    for (const name of Object.keys(schema.$defs)) reference(file, `#/$defs/${name}`);
  }
  return compile({ type: "object", definitions }, "ContractModel", {
    ...options,
    unreachableDefinitions: true,
    format: false,
  });
}
