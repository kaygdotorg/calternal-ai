// The web client's documentation gate: every exported function, class, type, interface (and the members of a multi-line interface), constant and store in the shell, appearance, settings, notes, palette and server folders and in packages/ui and
// packages/editor carries a `/** … */` comment, and every Svelte component opens with a comment that says what it is for. It reads the TypeScript compiler's syntax tree (no build) and counts what is missing.
// Usage: node scripts/check-docs.mjs [--list]    Exits 1 when anything is undocumented (the count is the report's "exported items without comments").
import ts from "typescript";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(fileURLToPath(new URL(".", import.meta.url)), "../../..");
const DIRS = ["apps/web/src/shell", "apps/web/src/appearance", "apps/web/src/apps/settings", "apps/web/src/apps/notes", "apps/web/src/palette", "apps/web/src/server", "packages/ui/src", "packages/editor/src"];
const files = (d) => readdirSync(d).flatMap((n) => { const p = join(d, n); return statSync(p).isDirectory() ? (n === "node_modules" ? [] : files(p)) : [p]; });
const missing = [];
const flag = (file, line, what) => missing.push(`${relative(root, file)}:${line}  ${what}`);
const documented = (node, text) => ts.getLeadingCommentRanges(text, node.getFullStart())?.some((r) => text.slice(r.pos, r.pos + 3) === "/**") ?? false;
const exported = (n) => (ts.getCombinedModifierFlags(n) & ts.ModifierFlags.Export) !== 0;

function scan(file, text, offset = 0) {
  const sf = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, file.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const line = (n) => sf.getLineAndCharacterOfPosition(n.getStart()).line + 1 + offset;
  for (const n of sf.statements) {
    if ((ts.isFunctionDeclaration(n) || ts.isClassDeclaration(n) || ts.isInterfaceDeclaration(n) || ts.isTypeAliasDeclaration(n) || ts.isEnumDeclaration(n)) && exported(n)) {
      if (!documented(n, text)) flag(file, line(n), `${n.name?.text ?? "default"}`);
      // A one-line interface is described by its own comment; a multi-line one documents each member on its own line.
      if (ts.isInterfaceDeclaration(n) && sf.getLineAndCharacterOfPosition(n.getStart()).line !== sf.getLineAndCharacterOfPosition(n.getEnd()).line) for (const m of n.members) if (!documented(m, text)) flag(file, line(m), `${n.name.text}.${m.name?.getText(sf) ?? "?"}`);
    } else if (ts.isVariableStatement(n) && exported(n)) {
      if (!documented(n, text)) flag(file, line(n), n.declarationList.declarations.map((d) => d.name.getText(sf)).join(", "));
    }
  }
}
for (const d of DIRS) for (const f of files(join(root, d))) {
  if (/\.test\.ts$|\.d\.ts$/.test(f)) continue;
  const text = readFileSync(f, "utf8");
  if (f.endsWith(".ts")) scan(f, text);
  else if (f.endsWith(".svelte")) {
    // A component opens its script with a comment (purpose, and what it takes); `<script module>` exports are checked like a module's.
    const m = /<script(?:\s[^>]*)?>\s*(\/\/|\/\*)/.exec(text);
    const scripts = [...text.matchAll(/<script(\s[^>]*)?>([\s\S]*?)<\/script>/g)];
    if (!scripts.length) continue;
    if (!/^\s*<script[^>]*>\s*(\/\/|\/\*)/m.test(text) && !/^\s*(\/\/|\/\*)/.test(scripts.find((s) => !/module/.test(s[1] ?? ""))?.[2]?.trim() ?? "")) flag(f, 1, "component has no opening comment");
    for (const s of scripts) if (/module/.test(s[1] ?? "")) scan(f + ".module.ts", s[2], text.slice(0, s.index).split("\n").length - 1);
  }
}
if (process.argv.includes("--list")) console.log(missing.join("\n"));
console.log(`${missing.length} undocumented exported items`);
process.exit(missing.length ? 1 : 0);
