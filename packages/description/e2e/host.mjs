// The renderer against the step 0 host: view open, windows, rows and states drawn from the host's own template. Needs the host reachable at HOST (default http://127.0.0.1:43125/step0/v2) and the bearer in the file
// named by HOST_BEARER_FILE (never printed). Usage: node e2e/host.mjs [home|shared] [screenshot.png]
import { readFileSync } from "node:fs";
import { launch, open, serve } from "./lib.mjs";
const HOST = process.env.HOST ?? "http://127.0.0.1:43125/step0/v2";
const bearer = readFileSync(process.env.HOST_BEARER_FILE ?? `${process.env.HOME}/Developer/calternal-ai-lanes/.owner.token`, "utf8").trim();
const view = process.argv[2] ?? "home", shot = process.argv[3];
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const { page: p } = await open(browser, `host=${encodeURIComponent(HOST)}&bearer=${encodeURIComponent(bearer)}&viewName=${view}`);
await p.waitForFunction(() => window.__painted !== undefined, null, { timeout: 15000 }).catch(() => {});
const r = await p.evaluate(() => ({
  painted: window.__painted, lists: [...document.querySelectorAll("[data-bind]")].map((l) => ({ bind: l.getAttribute("data-bind"), state: l.getAttribute("data-state"), count: Number(l.getAttribute("data-count")) })),
  titles: [...document.querySelectorAll("[data-i] .t")].slice(0, 8).map((e) => e.textContent), skeletons: document.querySelectorAll("[data-skeleton]").length,
}));
ok(r.painted !== undefined, `${view}: rows painted from the host`, `${r.painted?.toFixed(0)} ms after view open`);
ok(r.lists.some((l) => l.state === "ready" && l.count > 0), `${view}: a list is ready with the host's count`, JSON.stringify(r.lists));
ok(r.titles.length > 0 && r.titles.every((t) => t && t.length > 0), `${view}: every drawn row has a title`, r.titles.slice(0, 3).join(" | "));
console.log(`INFO  lists ${JSON.stringify(r.lists)}`);
if (shot) await p.screenshot({ path: shot });
await browser.close(); stop();
process.exit(bad ? 1 : 0);
