// Render the public example files from the sources copied by the CLI scaffold.
import { readFile, writeFile } from "node:fs/promises";

const guide = new URL("../", import.meta.url);
const repo = new URL("../../", guide);
const check = process.argv.includes("--check");
const templates = "packages/plugin-sdk/templates";
const fixtures = "crates/server/plugin-template";

const examples = [
  {
    page: "walkthrough.md", section: "view", name: "my-plugin", heading: "###",
    files: [
      ["manifest.json", "json", `${templates}/manifest.json`],
      ["worker.ts", "ts", `${fixtures}/worker.ts`],
      ["worker.test.mjs", "js", `${fixtures}/worker.test.mjs`],
    ],
  },
  {
    page: "reference.md", section: "question", name: "question-job", heading: "#####",
    files: [
      ["manifest.json", "json", `${templates}/question.manifest.json`],
      ["question.form.json", "json", `${templates}/question.form.json`],
      ["worker.ts", "ts", `${templates}/question.worker.txt`],
      ["worker.test.mjs", "js", `${fixtures}/question.test.mjs`],
    ],
  },
];

for (const { page, section, name, heading, files } of examples) {
  const destination = new URL(page, guide);
  const original = await readFile(destination, "utf8");
  const open = `<!-- BEGIN GENERATED ${section} -->`;
  const close = `<!-- END GENERATED ${section} -->`;
  const start = original.indexOf(open);
  const end = original.indexOf(close);
  if (start < 0 || end < start + open.length ||
      original.indexOf(open, start + open.length) !== -1 ||
      original.indexOf(close, end + close.length) !== -1) {
    throw new Error(`Expected one generated ${section} block in ${page}`);
  }

  const blocks = await Promise.all(files.map(async ([file, language, path]) => {
    const template = await readFile(new URL(path, repo), "utf8");
    const source = template.trimEnd().replaceAll("__PLUGIN__", name);
    return `${heading} ${file}\n\n\`\`\`${language}\n${source}\n\`\`\``;
  }));
  const next = original.slice(0, start + open.length) + "\n\n" +
    blocks.join("\n\n") + "\n\n" + original.slice(end);
  if (next === original) continue;
  if (check) {
    console.error(`${page} has stale scaffold examples. Run npm run docs:examples --workspace @calternal/plugin-guide.`);
    process.exitCode = 1;
  } else {
    await writeFile(destination, next);
  }
}
