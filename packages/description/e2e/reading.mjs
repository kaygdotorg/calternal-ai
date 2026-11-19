// The reading-list plugin end to end against the step 0 host (step 5): the description the host serves for `reading` is drawn, the Add form is filled and submitted, the host resolves and dispatches it (a plugin
// operation: two exchanges, no sheet), the outcome's `created` ref and a re-read of the list show the new row, and what crossed the wire is only the activation context and the typed form values.
// Needs the host at HOST (default http://127.0.0.1:43125/step0/v2) and the owner bearer in HOST_BEARER_FILE (never printed). Usage: node e2e/reading.mjs [screenshot.png]
import { readFileSync } from "node:fs";
import { launch, open, serve } from "./lib.mjs";
const HOST = process.env.HOST ?? "http://127.0.0.1:43125/step0/v2";
const bearer = readFileSync(process.env.HOST_BEARER_FILE ?? `${process.env.HOME}/Developer/calternal-ai-lanes/.owner.token`, "utf8").trim();
const shot = process.argv[2];
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const { page: p } = await open(browser, `host=${encodeURIComponent(HOST)}&bearer=${encodeURIComponent(bearer)}&viewName=reading`);
const wire = [];
p.on("request", (r) => { if (r.method() === "POST" && /activation|choice|undo|status/.test(r.url())) wire.push({ route: new URL(r.url()).pathname.split("/").pop(), body: r.postDataJSON() }); });
await p.waitForSelector("[data-form=add]", { timeout: 20000 });
await p.waitForSelector("[data-i]", { timeout: 20000 }).catch(() => {});
const before = await p.locator("[data-node=list] [data-i], [data-sidebar] [data-i]").count();
const title = `Fixture article ${Date.now() % 100000}`;
await p.locator("[data-form=add] label:has-text('Title') input").fill(title);
const submit = p.locator("[data-form=add] [data-action=add]");
ok((await submit.getAttribute("aria-disabled")) === "true", "Add is disabled while the address is empty");
await p.locator("[data-form=add] label:has-text('Address') input").fill("https://example.org/article");
ok((await submit.getAttribute("aria-disabled")) === null, "Add enables when the form is valid");
await submit.click();
await p.waitForFunction((t) => [...document.querySelectorAll("[data-i] .t")].some((e) => e.textContent === t), title, { timeout: 40000 }).catch(() => {});
const shown = await p.evaluate((t) => [...document.querySelectorAll("[data-i] .t")].some((e) => e.textContent === t), title);
ok(shown, "the new item is drawn from the host's next window", title);
const routes = wire.map((w) => w.route);
ok(routes.join() === "activation.resolve,activation.dispatch", "a plugin operation is resolve then dispatch, no sheet", routes.join(","));
const res = wire.find((w) => w.route === "activation.resolve")?.body ?? {};
ok(JSON.stringify(Object.keys(res).sort()) === JSON.stringify(["action_id", "activation_context", "activation_id", "interaction", "type"]), "the resolve carries only the context and device-held values", Object.keys(res).join(","));
ok(JSON.stringify(Object.keys(res.interaction ?? {})) === '["form"]' && res.interaction.form.title === title, "interaction.form holds the typed values");
const cleared = await p.evaluate(() => [...document.querySelectorAll("[data-form=add] input")].every((i) => i.value === ""));
ok(cleared, "the form cleared after ok");
const after = await p.locator("[data-node=list] [data-i], [data-sidebar] [data-i]").count();
console.log(`INFO  rows drawn before ${before}, after ${after}`);
if (shot) await p.screenshot({ path: shot });
await browser.close(); stop();
process.exit(bad ? 1 : 0);
