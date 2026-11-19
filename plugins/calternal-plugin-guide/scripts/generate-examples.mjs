// Keep the public walkthrough identical to the example the build validates.
import { readFile, writeFile } from "node:fs/promises";

const guide = new URL("../", import.meta.url);
const example = (await readFile(new URL("examples/first-list.ts", guide), "utf8")).trimEnd();
const walkthrough = new URL("walkthrough.md", guide);
const previous = await readFile(walkthrough, "utf8");
const block = /<!-- BEGIN GENERATED first-list -->[\s\S]*?<!-- END GENERATED first-list -->/g;
if ([...previous.matchAll(block)].length !== 1) {
  throw new Error("Expected one first-list example in the walkthrough.");
}
const current = previous.replace(block, () =>
  `<!-- BEGIN GENERATED first-list -->\n\n\`\`\`ts\n${example}\n\`\`\`\n\n<!-- END GENERATED first-list -->`);
if (current !== previous) {
  if (process.argv.includes("--check")) {
    throw new Error("The walkthrough is stale. Run npm run docs:examples -w @calternal/plugin-guide.");
  }
  await writeFile(walkthrough, current);
}
