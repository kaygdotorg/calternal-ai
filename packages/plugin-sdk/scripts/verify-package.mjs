import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFile, realpath } from "node:fs/promises";
import { isAbsolute, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import Ajv from "ajv";
import addFormats from "ajv-formats";

const contract = new URL("../generated/contract/", import.meta.url);
const readSchema = async name => JSON.parse(await readFile(new URL(name, contract), "utf8"));
const digest = bytes => createHash("sha256").update(bytes).digest("hex");

async function metadataValidator() {
  const manifest = await readSchema("plugin-manifest-v2.schema.json");
  const { $id, $defs } = await readSchema("host-catalogue-v2.json");
  const v1 = await readSchema("plugin-manifest-v1.schema.json");
  // The catalogue is a 2020-12 registry of definitions. Manifest v2 only
  // references its Draft 7-compatible definitions.
  const ajv = new Ajv({ strict: false, allErrors: true });
  addFormats(ajv);
  for (const bits of [8, 32]) {
    ajv.addFormat(`uint${bits}`, {
      type: "number",
      validate: value => Number.isInteger(value) && value >= 0 && value < 2 ** bits,
    });
  }
  ajv.addSchema({ $id, $defs });
  ajv.addSchema(v1, new URL("plugin-manifest-v1.schema.json", manifest.$id).href);
  ajv.addSchema(manifest);
  return ajv;
}

async function checkCreateDefaults(kinds) {
  if (!kinds.some(kind => kind.host_create)) return;
  const ajv = await metadataValidator();
  for (const [index, kind] of kinds.entries()) {
    if (!kind.host_create) continue;
    const checkItem = ajv.compile(kind.schema);
    if (!checkItem(kind.host_create.defaults)) {
      throw new Error(`manifest.json /kinds/${index}/host_create/defaults ${ajv.errorsText(checkItem.errors)}`);
    }
  }
}

async function readPinned(directory, { path, sha256 }) {
  const file = await realpath(join(directory, path));
  const inside = relative(directory, file);
  if (!inside || inside === ".." || inside.startsWith(".." + sep) || isAbsolute(inside)) {
    throw new Error(`Asset ${path} must resolve inside the package directory.`);
  }
  const bytes = await readFile(file);
  if (digest(bytes) !== sha256) throw new Error(`Asset ${path} does not match its SHA-256 pin.`);
  return bytes;
}

/**
 * Verify emitted package files using the manifest schema, asset digests and
 * sealed description verifier in package mode. Validates create defaults
 * and the worker pin, then returns pinned data for authoring checks.
 * A failed check throws with the underlying schema or artifact diagnostic.
 */
export async function verifyPackage(directory) {
  const root = await realpath(directory);
  try {
    const output = execFileSync(
      "python3", ["-B", fileURLToPath(new URL("verify-description.py", contract)), "--package", root],
      { encoding: "utf8" },
    );
    process.stdout.write(output);
  } catch (error) {
    const diagnostic = [error.stdout, error.stderr].filter(Boolean).join("").trim();
    if (diagnostic) throw new Error(diagnostic, { cause: error });
    throw error;
  }

  const manifest = JSON.parse(await readFile(join(root, "manifest.json"), "utf8"));
  await checkCreateDefaults(manifest.kinds);
  const apps = await Promise.all(manifest.apps.map(async app => ({
    app,
    bytes: await readPinned(root, app.description.template),
  })));
  if (manifest.worker) await readPinned(root, manifest.worker);
  return { manifest, apps: apps.map(({ app, bytes }) => ({ app, template: JSON.parse(bytes) })) };
}
