// The window chrome is shared and lives in src/canvas/Canvas.svelte: content scrolls to the card's top border under floating controls, with only a 6 pt fade that starts at the border. It has regressed into a top band twice (2026-11-24, in c2f7476b and 3b215ff1), so the build checks it. Run: node scripts/check-chrome.mjs (part of `npm run build`). It fails when:
//  1. Canvas.svelte's page scroller does not carry exactly the minimal top fade;
//  2. Canvas.svelte paints or masks anything else across the top bar (a ::before/::after on the bar, a background on it);
//  3. a window page (src/apps, src/canvas) adds a vertical mask of its own, or pins a sticky element to `top: 0` instead of `var(--sticky-top, 0px)` (which keeps it below the floating controls).
import { readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "src");
const FADE = "mask-image: linear-gradient(to bottom, transparent, #000 var(--s-3));";
const fails = [];
const walk = (d) => readdirSync(d).flatMap((f) => (statSync(join(d, f)).isDirectory() ? walk(join(d, f)) : [join(d, f)]));
const code = (p) => readFileSync(p, "utf8").replace(/\/\*[\s\S]*?\*\//g, "").replace(/<!--[\s\S]*?-->/g, "");

const canvas = code(join(root, "canvas", "Canvas.svelte"));
const scroller = canvas.split("\n").find((l) => /^\s*\.scroll \{ flex: 1;/.test(l)) ?? "";
if (!scroller.includes(`-webkit-${FADE}`) || !scroller.includes(` ${FADE}`)) fails.push("canvas/Canvas.svelte: the page scroller must carry exactly the minimal top fade: " + FADE);
if (!canvas.includes(":global(.ub:not(.scroll)) { padding-top: var(--bar-h)") || !canvas.includes(":global(.scroll.ub) { padding-top: 0; }")) fails.push("canvas/Canvas.svelte: a page whose first element is the scroller must still be extended to the card's top border (.ub rules)");
if (/\.wh[^{,]*::(before|after)/.test(canvas)) fails.push("canvas/Canvas.svelte: no pseudo-element across the window's top bar (it reads as a band)");
if (/(^|\n)\s*\.wh(:not\([^)]*\))?(\.[a-z-]+)* \{[^}]*(background|backdrop-filter|mask-image)\s*:/.test(canvas)) fails.push("canvas/Canvas.svelte: the window's top bar has no fill, blur or mask of its own");

for (const dir of ["apps", "canvas"]) for (const file of walk(join(root, dir))) {
  if (!/\.(svelte|css)$/.test(file)) continue;
  const s = code(file), name = relative(root, file);
  if (name !== join("canvas", "Canvas.svelte") && /mask-image\s*:\s*linear-gradient\(\s*to (bottom|top)/.test(s)) fails.push(`${name}: a window page adds no vertical mask; the chrome is Canvas.svelte's`);
  if (/position:\s*sticky;[^}]*\btop:\s*0(px)?\s*[;}]/.test(s)) fails.push(`${name}: a sticky header pins at var(--sticky-top, 0px), never top: 0 (it would sit under the window's controls)`);
}

if (fails.length) { console.error("window chrome check failed:\n" + fails.map((f) => "  - " + f).join("\n")); process.exit(1); }
console.log("window chrome check: ok");
