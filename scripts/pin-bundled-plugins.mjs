/** Derive hashes from built bundles in the explicitly reviewed distribution seed. */
import { createHash } from "node:crypto";
import { readFile, rename, unlink, writeFile } from "node:fs/promises";

const pin = new URL("../crates/server/trusted-plugins.json", import.meta.url);
const root = new URL("../plugins/", import.meta.url);
const args = process.argv.slice(2);
if (args.length && (args.length !== 1 || args[0] !== "--check")) {
  throw new Error("Usage: npm run pin:plugins -- [--check]");
}

const seed = JSON.parse(await readFile(pin, "utf8"));
const changed = [];
for (const entry of seed.plugins) {
  const digest = entry.worker == null
    ? null
    : createHash("sha256").update(await readFile(new URL(entry.worker, root))).digest("hex");
  if (digest !== entry.worker_sha256) {
    changed.push(entry.id);
    entry.worker_sha256 = digest;
  }
}
if (changed.length && args[0] === "--check") {
  throw new Error(`Worker pins differ for ${changed.join(", ")}. Rebuild and run npm run pin:plugins.`);
}
if (changed.length) {
  const next = new URL(`./trusted-plugins.json.next-${process.pid}`, pin);
  await writeFile(next, JSON.stringify(seed, null, 2) + "\n", { flag: "wx" });
  try {
    await rename(next, pin);
  } catch (error) {
    await unlink(next);
    throw error;
  }
}
console.log(changed.length ? `Updated pins for ${changed.join(", ")}.` : "Bundled worker pins match.");
