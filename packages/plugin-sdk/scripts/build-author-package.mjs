#!/usr/bin/env node
// Offline entry point for the SDK's build command. Never starts the package worker.
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, realpath, rename, rm, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { readPinned, verifyPackage } from "./verify-package.mjs";

const bytesOf = value => Buffer.from(JSON.stringify(value, null, 2) + "\n");
const digest = bytes => createHash("sha256").update(bytes).digest("hex");

async function build(source, output, workerPath) {
  if (!source || !output || process.argv.length > 5) {
    throw new Error("Usage: node build-author-package.mjs <plugin.ts> <output-dir> [bundled-worker.mjs]");
  }
  const { default: plugin } = await import(pathToFileURL(resolve(source)).href);
  const apps = plugin.apps.map(({ metadata, template }, index) => {
    const bytes = bytesOf(template);
    const sha256 = digest(bytes);
    const asset = { path: `app-${index}-${sha256}.json`, sha256 };
    const app = { ...metadata, description: { id: "app", template: asset } };
    return { app, asset, bytes };
  });
  const worker = workerPath && await readFile(workerPath);
  const workerHash = worker && digest(worker);
  const workerAsset = workerHash && { path: `worker-${workerHash}.mjs`, sha256: workerHash };
  const assets = apps.map(({ asset, bytes }) => ({ path: asset.path, bytes }));
  if (workerAsset) assets.push({ path: workerAsset.path, bytes: worker });
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
  const manifest = bytesOf({ ...plugin.metadata, apps: apps.map(entry => entry.app), worker: workerAsset, contributions });
  // Validate the emitted bytes before publishing the manifest.
  await mkdir(output, { recursive: true });
  const staging = await mkdtemp(join(output, ".author-build-"));
  try {
    await Promise.all([
      ...assets.map(asset => writeFile(join(staging, asset.path), asset.bytes)),
      writeFile(join(staging, "manifest.json"), manifest),
    ]);
    await verifyPackage(staging);
    for (const asset of assets) await rename(join(staging, asset.path), join(output, asset.path));
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
