// A frame sequence of pinning a conversation by dragging it onto the strip (fixture data): press, lift, travel, over the strip (it opens and lights), drop, settled. Saves PNGs in the directory given; one run per scheme.
// Usage: SCHEME=light|dark node e2e/sidebar-frames.mjs <directory>
import { mkdirSync } from "node:fs";
import { launch, open, serve } from "./lib.mjs";
const dir = process.argv[2] ?? "frames"; mkdirSync(dir, { recursive: true });
const sc = process.env.SCHEME ?? "light";
const stop = await serve(); const browser = await launch();
const { page: p } = await open(browser, `fixture=threads-next&scheme=${sc}`, { width: 420, height: 760 });
await p.waitForSelector("[data-tree] .sh"); await p.waitForTimeout(900);
const shot = async (n) => { await p.waitForTimeout(120); await p.screenshot({ path: `${dir}/${sc}-frame-${n}.png`, clip: { x: 0, y: 0, width: 300, height: 760 } }); };
const c = async (l) => { const b = await l.boundingBox(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 }; };
const row = p.locator('[data-in="groups"][data-row-id="t-design"]'), strip = p.locator("[data-strip=pins]");
const a = await c(row), z = await c(strip);
await shot("1-rest");
await p.mouse.move(a.x, a.y); await p.mouse.down(); await shot("2-press");
for (let i = 1; i <= 4; i++) await p.mouse.move(a.x, a.y + (z.y - a.y) * (i / 12)), await p.waitForTimeout(16);
await shot("3-lift");
for (let i = 5; i <= 12; i++) { await p.mouse.move(a.x + (z.x - a.x) * (i / 12), a.y + (z.y - a.y) * (i / 12)); await p.waitForTimeout(16); }
await shot("4-over-strip");
await p.mouse.up(); await p.waitForTimeout(80); await shot("5-drop");
await p.waitForTimeout(700); await shot("6-settled");
await browser.close(); stop();
