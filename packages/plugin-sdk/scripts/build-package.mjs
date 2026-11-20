// Emit one plugin package offline: manifest, pinned templates, worker and
// harness entries, verified before publication. Never starts the worker.
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, realpath, rename, rm, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { readPinned, verifyPackage } from "./verify-package.mjs";

const bytesOf = value => Buffer.from(JSON.stringify(value, null, 2) + "\n");
const digest = bytes => createHash("sha256").update(bytes).digest("hex");

/**
 * Build the package that `source` default-exports through `definePlugin` into
 * `output`. `worker`, when given, is the already bundled worker module. Writes
 * into a staging directory, verifies it, then renames the files into place.
 * Returns the emitted manifest, the asset list and the verifier's report.
 */
export async function buildPackage(source, output, worker) {
  const { default: plugin } = await import(pathToFileURL(resolve(source)).href);
  if (!plugin?.metadata || !Array.isArray(plugin.apps)) throw new Error(`${source} must default-export the result of definePlugin.`);
  const apps = plugin.apps.map(({ metadata, template }, index) => {
    const bytes = bytesOf(template);
    const sha256 = digest(bytes);
    const asset = { path: `app-${index}-${sha256}.json`, sha256 };
    return { app: { ...metadata, description: { id: "app", template: asset } }, asset, bytes };
  });
  const workerBytes = worker && await readFile(worker);
  const workerAsset = workerBytes && { path: `worker-${digest(workerBytes)}.mjs`, sha256: digest(workerBytes) };
  const assets = apps.map(({ asset, bytes }) => ({ path: asset.path, bytes }));
  if (workerAsset) assets.push({ path: workerAsset.path, bytes: workerBytes });
  const sourceDirectory = await realpath(dirname(resolve(source)));
  const entries = new Map();
  const contributions = plugin.metadata.contributions && await Promise.all(
    plugin.metadata.contributions.map(async contribution => {
      if (!contribution.harness) return contribution;
      const original = contribution.harness.entry;
      const bytes = await readPinned(sourceDirectory, original);
      const entry = { path: `entry-${original.sha256}.bin`, sha256: original.sha256 };
      entries.set(entry.path, { path: entry.path, bytes });
      return { ...contribution, harness: { ...contribution.harness, entry } };
    }),
  );
  assets.push(...entries.values());
  const manifest = { ...plugin.metadata, apps: apps.map(entry => entry.app), worker: workerAsset, contributions };
  await mkdir(output, { recursive: true });
  const staging = await mkdtemp(join(output, ".author-build-"));
  try {
    await Promise.all([
      ...assets.map(asset => writeFile(join(staging, asset.path), asset.bytes)),
      writeFile(join(staging, "manifest.json"), bytesOf(manifest)),
    ]);
    const { report } = await verifyPackage(staging);
    for (const asset of assets) await rename(join(staging, asset.path), join(output, asset.path));
    await rename(join(staging, "manifest.json"), join(output, "manifest.json"));
    return { manifest: JSON.parse(bytesOf(manifest)), assets: assets.map(({ path, bytes }) => ({ path, bytes: bytes.length })), report };
  } finally {
    await rm(staging, { recursive: true, force: true });
  }
}
