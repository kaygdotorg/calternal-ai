/**
 * Shared window-chrome gate (ported from apps/mockup/scripts/check-chrome.mjs):
 * Verifies that Canvas.svelte keeps the exact 6 pt top fade with no top band,
 * and that no window page in any checked directory adds a vertical mask or top:0 sticky.
 */
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const pkgRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(pkgRoot, "../..");
const FADE = "mask-image: linear-gradient(to bottom, transparent, #000 var(--s-3));";
const fails = [];
const walk = (d) =>
  existsSync(d)
    ? readdirSync(d).flatMap((f) => (statSync(join(d, f)).isDirectory() ? walk(join(d, f)) : [join(d, f)]))
    : [];
const code = (p) => readFileSync(p, "utf8").replace(/\/\*[\s\S]*?\*\//g, "").replace(/<!--[\s\S]*?-->/g, "");

const canvasPath = join(pkgRoot, "src", "canvas", "Canvas.svelte");
const canvas = code(canvasPath);
const scroller = canvas.split("\n").find((l) => /^\s*\.scroll \{ flex: 1;/.test(l)) ?? "";
if (!scroller.includes(`-webkit-${FADE}`) || !scroller.includes(` ${FADE}`)) {
  fails.push("canvas/Canvas.svelte: the page scroller must carry exactly the minimal top fade: " + FADE);
}
if (!canvas.includes(":global(.ub:not(.scroll)) { padding-top: var(--bar-h)") || !canvas.includes(":global(.scroll.ub) { padding-top: 0; }")) {
  fails.push("canvas/Canvas.svelte: a page whose first element is the scroller must still be extended to the card's top border (.ub rules)");
}
if (/\.wh[^{,]*::(before|after)/.test(canvas)) {
  fails.push("canvas/Canvas.svelte: no pseudo-element across the window's top bar (it reads as a band)");
}
if (/(^|\n)\s*\.wh(:not\([^)]*\))?(\.[a-z-]+)* \{[^}]*(background|backdrop-filter|mask-image)\s*:/.test(canvas)) {
  fails.push("canvas/Canvas.svelte: the window's top bar has no fill, blur or mask of its own");
}

const dirs = process.argv.slice(2).length
  ? process.argv.slice(2).map((d) => resolve(d))
  : [join(repoRoot, "apps/mockup/src/apps"), join(repoRoot, "apps/mockup/src/canvas"), join(repoRoot, "apps/web/src/host")];

for (const dir of dirs) {
  for (const file of walk(dir)) {
    if (!/\.(svelte|css)$/.test(file)) continue;
    const s = code(file);
    const name = relative(repoRoot, file);
    if (resolve(file) !== canvasPath && /mask-image\s*:\s*linear-gradient\(\s*to (bottom|top)/.test(s)) {
      // Sidebar bodies intentionally carry their own top/bottom fade; only window pages are forbidden.
      if (!/Sidebar/.test(name)) {
        fails.push(`${name}: a window page adds no vertical mask; the chrome is Canvas.svelte's`);
      }
    }
    if (/position:\s*sticky;[^}]*\btop:\s*0(px)?\s*[;}]/.test(s)) {
      fails.push(`${name}: a sticky header pins at var(--sticky-top, 0px), never top: 0`);
    }
  }
}

if (fails.length) {
  console.error("window chrome check failed:\n" + fails.map((f) => "  - " + f).join("\n"));
  process.exit(1);
}
console.log("window chrome check: ok");
