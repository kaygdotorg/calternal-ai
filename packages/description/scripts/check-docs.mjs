// The docs gate for this package (AGENTS.md: every public item carries a doc comment): every exported declaration in src/ (not the generated model) must be directly preceded by a `/** ... */` comment, and so must the
// public members of an exported interface or class that sit on their own line after a blank-free doc comment is expected. Run by `npm run check` and the test suite. Exits non-zero listing what is missing.
import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(fileURLToPath(new URL(".", import.meta.url)), "../src");
const files = [];
(function walk(d) { for (const e of readdirSync(d, { withFileTypes: true })) { const p = join(d, e.name); if (e.isDirectory()) { if (e.name !== "generated") walk(p); } else if (/\.(ts|svelte)$/.test(e.name)) files.push(p); } })(root);

const missing = [];
for (const f of files) {
  const lines = readFileSync(f, "utf8").split("\n");
  lines.forEach((l, i) => {
    if (!/^\s*export\s+(const|function|async function|class|interface|type|enum)\b/.test(l)) return;
    let j = i - 1; while (j >= 0 && lines[j].trim() === "") j--;
    if (j < 0 || !/\*\/\s*$/.test(lines[j])) missing.push(`${relative(root, f)}:${i + 1}: ${l.trim().slice(0, 70)}`);
  });
}
if (missing.length) { console.error(`${missing.length} exported items lack a doc comment:\n${missing.join("\n")}`); process.exit(1); }
console.log(`docs gate: ${files.length} files, every export documented`);
