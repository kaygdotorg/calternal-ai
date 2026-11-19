import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { copyFile, mkdir, mkdtemp, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { isAbsolute, join, relative, sep } from "node:path";
import Ajv from "ajv";
import addFormats from "ajv-formats";

const contract = new URL("../generated/contract/", import.meta.url);
const readSchema = async name => JSON.parse(await readFile(new URL(name, contract), "utf8"));
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

async function checkDescriptions(apps, manifest) {
  if (!apps.length) return;
  // Adapt package files to the fixture layout expected by the sealed verifier.
  const staging = await mkdtemp(join(tmpdir(), "calternal-descriptions-"));
  try {
    const fixtures = join(staging, "fixtures", "description-v2");
    const manifests = join(fixtures, "manifests");
    await mkdir(manifests, { recursive: true });
    await Promise.all([
      ...["verify-description.py", "description-v2.schema.json", "host-catalogue-v2.json"].map(
        file => copyFile(new URL(file, contract), join(staging, file)),
      ),
      ...apps.flatMap(({ bytes }, index) => {
        const filename = `app-${index}.json`;
        return [
          writeFile(join(fixtures, filename), bytes),
          writeFile(join(manifests, filename), manifest),
        ];
      }),
    ]);
    execFileSync("python3", [join(staging, "verify-description.py")], {
      env: { ...process.env, CALTERNAL_HOST_CATALOGUE: join(staging, "host-catalogue-v2.json") },
      stdio: "inherit",
    });
  } finally {
    await rm(staging, { recursive: true, force: true });
  }
}

/**
 * Verify emitted package files using the manifest schema, asset digests and
 * sealed description verifier. Reads each app template and optional worker
 * once, invokes Python once and returns the checked manifest and app templates.
 * A failed check throws with the underlying schema or artifact diagnostic.
 */
export async function verifyPackage(directory) {
  const root = await realpath(directory);
  const manifestBytes = await readFile(join(root, "manifest.json"));
  const manifest = JSON.parse(manifestBytes);
  checkManifest(manifest, await manifestValidator());
  const apps = await Promise.all(manifest.apps.map(async app => ({
    app,
    bytes: await readPinned(root, app.description.template),
  })));
  if (manifest.worker) await readPinned(root, manifest.worker);
  await checkDescriptions(apps, manifestBytes);
  return { manifest, apps: apps.map(({ app, bytes }) => ({ app, template: JSON.parse(bytes) })) };
}
