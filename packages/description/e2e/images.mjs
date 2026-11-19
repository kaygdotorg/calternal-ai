// Row images (revision 13): Notes rows that the host gave a thumbnail draw the host's image (read through the row-image route under the view's lease); the others draw the snippet miniature, never a blank.
// Needs the step 0 host (HOST, HOST_BEARER_FILE). Usage: node e2e/images.mjs [screenshot.png]
import { readFileSync } from "node:fs";
import { launch, open, serve } from "./lib.mjs";
const HOST = process.env.HOST ?? "http://127.0.0.1:43125/step0/v2";
const bearer = readFileSync(process.env.HOST_BEARER_FILE ?? `${process.env.HOME}/Developer/calternal-ai-lanes/.owner.token`, "utf8").trim();
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const { page: p } = await open(browser, `host=${encodeURIComponent(HOST)}&bearer=${encodeURIComponent(bearer)}&viewName=notes`);
const reads = []; p.on("response", (r) => { if (/row-images/.test(r.url())) reads.push({ status: r.status(), type: r.headers()["content-type"], lease: r.request().headers()["x-step0-view-lease"] }); });
await p.waitForSelector("[data-section=notes] [data-row-image]", { timeout: 60000 });
await p.waitForTimeout(4000);
const r = await p.evaluate(() => { const rows = [...document.querySelectorAll("[data-section=notes] [data-i]")].slice(0, 12); return { n: rows.length, img: rows.filter((e) => e.querySelector("[data-row-image] img")).length, mini: rows.filter((e) => e.querySelector("[data-miniature]")).length, none: rows.filter((e) => !e.querySelector("[data-row-image] img, [data-miniature]")).length }; });
ok(r.n >= 6 && r.none === 0, "every one of the first rows draws an image or a miniature, never a blank", JSON.stringify(r));
ok(r.img >= 1 && r.mini >= 1, "some rows draw the host's thumbnail and the others the snippet miniature", JSON.stringify(r));
ok(reads.length > 0 && reads.every((x) => x.status === 200 && /image\/(png|jpeg)/.test(x.type ?? "") && !!x.lease), "image reads are authorised GETs under the lease and return png or jpeg", JSON.stringify(reads.slice(0, 2)));
if (process.argv[2]) await p.screenshot({ path: process.argv[2] });
await browser.close(); stop();
process.exit(bad ? 1 : 0);
