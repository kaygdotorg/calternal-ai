import { readFile } from "node:fs/promises";
import { join, sep } from "node:path";
import { pathToFileURL } from "node:url";

/** Resolve an explicit local authoring pin and reject a mismatched revision. */
export async function contractPin(defaultDirectory) {
  const selected = process.env.CALTERNAL_PLUGIN_CONTRACT;
  if (!selected) return defaultDirectory;
  const directory = pathToFileURL(join(selected, sep));
  const { exchanges_revision: revision } = JSON.parse(
    await readFile(new URL("host-catalogue-v2.json", directory), "utf8"),
  );
  if (process.env.CALTERNAL_PLUGIN_CONTRACT_REVISION !== revision) {
    throw new Error(`Explicit contract pin must match revision ${revision}.`);
  }
  return directory;
}
