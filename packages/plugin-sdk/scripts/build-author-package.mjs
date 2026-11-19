#!/usr/bin/env node
// Offline entry point for the SDK's build command. Never starts the package worker.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { copyFile, mkdir, mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import Ajv from "ajv";
import addFormats from "ajv-formats";

const contract = new URL("../../../contract/", import.meta.url);
const readSchema = async name => JSON.parse(await readFile(new URL(name, contract), "utf8"));
const bytesOf = value => Buffer.from(JSON.stringify(value, null, 2) + "\n");
const digest = bytes => createHash("sha256").update(bytes).digest("hex");

async function manifestValidator() {
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
  return { ajv, validate: ajv.compile(manifest) };
}

function checkManifest(manifest, { ajv, validate }) {
  if (!validate(manifest)) {
    throw new Error(`manifest.json ${ajv.errorsText(validate.errors, { separator: "\nmanifest.json " })}`);
  }
  for (const key of ["apps", "kinds", "operations"]) {
    const seen = new Set();
    for (const [index, entry] of manifest[key].entries()) {
      if (seen.has(entry.id)) throw new Error(`manifest.json /${key}/${index}/id repeats ${entry.id}`);
      seen.add(entry.id);
    }
  }
  for (const [index, kind] of manifest.kinds.entries()) {
    const checkItem = ajv.compile(kind.schema);
    if (kind.host_create && !checkItem(kind.host_create.defaults)) {
      throw new Error(`manifest.json /kinds/${index}/host_create/defaults ${ajv.errorsText(checkItem.errors)}`);
    }
  }
  for (const operation of manifest.operations) {
    ajv.compile(operation.input_schema);
    ajv.compile(operation.output_schema);
  }
}

async function checkDescriptions(apps, manifest) {
  if (!apps.length) return;
  // The sealed verifier reads a fixture directory, with each manifest next
  // to its corresponding description. Stage that layout in a private temp
  // directory so parallel builds never touch the checked-in fixtures.
  const staging = await mkdtemp(join(tmpdir(), "calternal-descriptions-"));
  try {
    const fixtures = join(staging, "fixtures", "description-v2");
    const manifests = join(fixtures, "manifests");
    await mkdir(manifests, { recursive: true });
    await Promise.all([
      ...["verify-description.py", "description-v2.schema.json", "host-catalogue-v2.json"].map(
        file => copyFile(new URL(file, contract), join(staging, file)),
      ),
      ...apps.flatMap(({ asset, bytes }) => [
        writeFile(join(fixtures, asset.path), bytes),
        writeFile(join(manifests, asset.path), manifest),
      ]),
    ]);
    execFileSync("python3", [join(staging, "verify-description.py")], {
      env: { ...process.env, CALTERNAL_HOST_CATALOGUE: join(staging, "host-catalogue-v2.json") },
      stdio: "inherit",
    });
  } finally {
    await rm(staging, { recursive: true, force: true });
  }
}

async function build(source, output, workerPath) {
  if (!source || !output || process.argv.length > 5) {
    throw new Error("Usage: node build-author-package.mjs <plugin.ts> <output-dir> [bundled-worker.mjs]");
  }
  const { default: plugin } = await import(pathToFileURL(resolve(source)).href);
  const apps = plugin.apps.map(({ metadata, template }) => {
    const bytes = bytesOf(template);
    const sha256 = digest(bytes);
    const asset = { path: `app-${metadata.id}-${sha256}.json`, sha256 };
    const app = { ...metadata, description: { id: "app", template: asset } };
    return { app, asset, bytes };
  });
  const worker = workerPath && await readFile(workerPath);
  const workerHash = worker && digest(worker);
  const workerAsset = workerHash && { path: `worker-${workerHash}.mjs`, sha256: workerHash };
  const manifest = bytesOf({ ...plugin.metadata, apps: apps.map(entry => entry.app), worker: workerAsset });
  checkManifest(JSON.parse(manifest), await manifestValidator());
  await checkDescriptions(apps, manifest);

  // Publish a complete manifest only after its immutable assets exist.
  await mkdir(output, { recursive: true });
  const staging = await mkdtemp(join(output, ".author-build-"));
  try {
    const assets = apps.map(({ asset, bytes }) => ({ path: asset.path, bytes }));
    if (workerAsset) assets.push({ path: workerAsset.path, bytes: worker });
    for (const asset of assets) {
      await writeFile(join(staging, asset.path), asset.bytes);
      await rename(join(staging, asset.path), join(output, asset.path));
    }
    await writeFile(join(staging, "manifest.json"), manifest);
    await rename(join(staging, "manifest.json"), join(output, "manifest.json"));
  } finally {
    await rm(staging, { recursive: true, force: true });
  }
}

try {
  await build(...process.argv.slice(2));
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
