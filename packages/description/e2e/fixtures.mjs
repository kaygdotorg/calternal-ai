// The test page suite (step 4): every VALID description fixture of contract/fixtures/description-v2 is drawn by the renderer in a real browser over the in-page mock host, and what the description declares must be on the
// screen: its sidebar sections, selectors, footer, panel, every list, form, button and host component, each list out of `loading` and not in `error`, with no script error. Invalid fixtures are the verifier's; a renderer
// only refuses what it cannot draw (version, requires), which the unit suite pins. Screenshots of each go to SHOTS (default none). Usage: node e2e/fixtures.mjs [shots-dir]
import { readdirSync, readFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { launch, open, serve } from "./lib.mjs";
const dir = join(dirname(fileURLToPath(import.meta.url)), "../../../contract/fixtures/description-v2");
const shots = process.argv[2]; if (shots) mkdirSync(shots, { recursive: true });
const names = readdirSync(dir).filter((n) => n.endsWith(".json") && !/^(invalid-|event-|reply-)/.test(n)).map((n) => n.replace(/\.json$/, ""))
  .filter((n) => { const d = JSON.parse(readFileSync(join(dir, `${n}.json`), "utf8")); return d.kind === "app" && d.version === 2; });
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { if (!c) { console.log(`FAIL  ${m}${note ? "  " + note : ""}`); bad++; } return c; };
const nodes = (n, out = []) => { if (!n) return out; out.push(n); for (const c of n.children ?? []) nodes(c, out); return out; };
let drawn = 0;
for (const name of names) {
  const d = JSON.parse(readFileSync(join(dir, `${name}.json`), "utf8"));
  const { page: p } = await open(browser, `mock=30&fixture=${name}`, { width: 1400, height: 900 });
  const errors = []; p.on("pageerror", (e) => errors.push(String(e))); p.on("console", (m) => { if (m.type() === "error") errors.push(m.text()); });
  await p.waitForSelector("[data-app], [data-refused]", { timeout: 15000 }).catch(() => {});
  await p.waitForTimeout(2800);
  const r = await p.evaluate(() => ({
    refused: !!document.querySelector("[data-refused]"), todo: document.querySelectorAll("[data-node-todo]").length,
    sections: [...document.querySelectorAll("[data-section] > h3")].map((e) => e.textContent), selectors: document.querySelectorAll("[data-selector]").length,
    footer: !!document.querySelector("[data-footer]"), apps: document.querySelectorAll("[data-app-entry]").length, panel: !!document.querySelector("[data-panel]"),
    comps: [...document.querySelectorAll("[data-host-component]")].map((e) => e.getAttribute("data-host-component")), forms: document.querySelectorAll("[data-node=form]").length, buttons: document.querySelectorAll("[data-node=button]").length,
    lists: [...document.querySelectorAll(".list[data-state]")].map((e) => e.getAttribute("data-state")),
  }));
  const tag = `${name}`;
  ok(!r.refused, `${tag}: drawn, not refused`);
  ok(r.todo === 0, `${tag}: no node left undrawn`, String(r.todo));
  ok(errors.length === 0, `${tag}: no script error`, errors[0]);
  const sb = d.chrome?.sidebar;
  for (const s of sb?.sections ?? []) if (s.title && !s.showWhen && !s.repeat) ok(r.sections.includes(s.title), `${tag}: section "${s.title}" drawn`, r.sections.join("|"));
  ok(r.selectors === (sb?.selectors?.length ?? 0), `${tag}: selectors`, `${r.selectors} of ${sb?.selectors?.length ?? 0}`);
  ok(r.footer === !!d.chrome?.footer, `${tag}: footer`);
  ok(r.apps === (d.chrome?.footer?.apps?.length ?? 0), `${tag}: footer apps`, `${r.apps}`);
  ok(r.panel === !!(d.chrome?.panel && d.chrome.panel.open !== false), `${tag}: panel`);
  const want = nodes(d.view?.root);
  ok(r.comps.length >= want.filter((n) => n.type === "component").length + (d.chrome?.panel?.components?.length ?? 0), `${tag}: host components`, r.comps.join(","));
  ok(r.forms === want.filter((n) => n.type === "form").length, `${tag}: forms`);
  ok(r.buttons === want.filter((n) => n.type === "button").length, `${tag}: buttons`);
  ok(!r.lists.includes("error") && !r.lists.includes("loading"), `${tag}: every list is out of loading and not in error`, r.lists.join(","));
  drawn++;
  if (shots) await p.screenshot({ path: join(shots, `${name}.png`) });
  await p.context().close();
}
console.log(`${bad ? "FAIL" : "PASS"}  ${drawn} valid fixtures drawn: ${names.join(", ")}`);
await browser.close(); stop();
process.exit(bad ? 1 : 0);
