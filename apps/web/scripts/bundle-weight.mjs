// Which source files make up a built chunk, by minified bytes (decoded from its source map). Build with sourcemaps first:
//   npx vite build --sourcemap --outDir /tmp/dist-map    then    node scripts/bundle-weight.mjs /tmp/dist-map/assets/index-*.js [topN]
import { readFileSync } from "node:fs";
const [file, top = "40"] = process.argv.slice(2);
const code = readFileSync(file, "utf8"); const map = JSON.parse(readFileSync(file + ".map", "utf8"));
const B64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const dec = (s) => { const out = []; let shift = 0, val = 0; for (const ch of s) { const d = B64.indexOf(ch); val += (d & 31) << shift; if (d & 32) shift += 5; else { out.push(val & 1 ? -(val >> 1) : val >> 1); val = shift = 0; } } return out; };
const lines = code.split("\n"); const mapLines = map.mappings.split(";");
const bytes = new Map(); let src = 0, ol = 0, oc = 0;
mapLines.forEach((ln, li) => {
  let col = 0; const segs = ln ? ln.split(",").map(dec) : [];
  segs.forEach((s, i) => { col += s[0]; if (s.length > 1) src += s[1]; const next = segs[i + 1] ? col + segs[i + 1][0] : (lines[li]?.length ?? col); const name = map.sources[src]?.replace(/^.*node_modules\//, "nm/").replace(/^(\.\.\/)+/, "") ?? "?"; if (s.length > 1) bytes.set(name, (bytes.get(name) ?? 0) + Math.max(0, next - col)); });
});
const total = [...bytes.values()].reduce((a, c) => a + c, 0);
console.log(`total mapped ${(total / 1024).toFixed(1)} KB of ${(code.length / 1024).toFixed(1)} KB`);
const group = new Map(); for (const [k, v] of bytes) { const g = k.startsWith("nm/") ? k.split("/").slice(0, k.split("/")[1]?.startsWith("@") ? 3 : 2).join("/") : k.replace(/\/[^/]*$/, "/*"); group.set(g, (group.get(g) ?? 0) + v); }
console.log([...group.entries()].sort((a, c) => c[1] - a[1]).slice(0, +top).map(([k, v]) => `${(v / 1024).toFixed(1).padStart(7)} KB  ${k}`).join("\n"));
