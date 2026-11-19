// Keep the walkthrough identical to the examples the guide build validates.
import { readFile, writeFile } from "node:fs/promises";

const guide = new URL("../", import.meta.url);
const walkthrough = new URL("walkthrough.md", guide);
const previous = await readFile(walkthrough, "utf8");
let current = previous;
for (const [name, filename, language] of [
  ["first-list", "first-list.ts", "ts"],
  ["gear-list", "gear-list.ts", "ts"],
  ["gear-worker", "gear-worker.mjs", "js"],
]) {
  const example = (await readFile(new URL(`examples/${filename}`, guide), "utf8")).trimEnd();
  const open = `<!-- BEGIN GENERATED ${name} -->`;
  const close = `<!-- END GENERATED ${name} -->`;
  const start = current.indexOf(open);
  const end = current.indexOf(close, start + open.length);
  if (start === -1 || end === -1) throw new Error(`Missing ${name} markers in the walkthrough.`);
  current = current.slice(0, start + open.length)
    + `\n\n\`\`\`${language}\n${example}\n\`\`\`\n\n`
    + current.slice(end);
}
if (current !== previous) {
  if (process.argv.includes("--check")) {
    throw new Error("The walkthrough is stale. Run npm run docs:examples -w @calternal/plugin-guide.");
  }
  await writeFile(walkthrough, current);
}
