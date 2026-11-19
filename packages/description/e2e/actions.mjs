// Sheets, swipe and Undo in a real browser against the step 0 host's Notes view (a development fixture; its data is disposable):
//  - a touch full swipe on a row runs the first leading action (Pin, catalogue-approved) in ONE exchange with dispatch:true, shows an Undo, and Undo goes through the host's undo route;
//  - Move to collection: the choice picker shows the host's labels, the resolve carries only the picked ChoiceRef, and when the host challenges, the protected confirmation sheet shows the host's material, Cancel sends nothing
//    more, Confirm sends invoke_confirm and then the dispatch with the proof;
//  - Rename: the prompt sheet answers go in interaction.prompt.
// Needs the host at HOST and the owner bearer in HOST_BEARER_FILE (never printed). Usage: node e2e/actions.mjs [screenshot-prefix]
import { readFileSync } from "node:fs";
import { launch, serve, BASE } from "./lib.mjs";
const HOST = process.env.HOST ?? "http://127.0.0.1:43125/step0/v2";
const bearer = readFileSync(process.env.HOST_BEARER_FILE ?? `${process.env.HOME}/Developer/calternal-ai-lanes/.owner.token`, "utf8").trim();
const shot = process.argv[2];
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const ctx = await browser.newContext({ viewport: { width: 1100, height: 800 }, hasTouch: true });
const p = await ctx.newPage(); const cdp = await ctx.newCDPSession(p);
const wire = [];
p.on("request", (r) => { if (r.method() === "POST" && /activation|choice|undo/.test(r.url())) wire.push({ route: new URL(r.url()).pathname.split("/").pop(), body: r.postDataJSON() }); });
const replies = []; p.on("response", async (r) => { if (/activation\.resolve/.test(r.url())) replies.push(await r.json().catch(() => ({}))); });
await p.goto(`${BASE}/?host=${encodeURIComponent(HOST)}&bearer=${encodeURIComponent(bearer)}&viewName=notes`);
const row = p.locator("[data-section=notes] [data-i]").first();
await row.waitFor({ timeout: 60000 });
await p.waitForTimeout(800);

// ---- touch full swipe, to the right (leading action: Pin)
const box = await row.boundingBox();
const y = box.y + box.height / 2, x0 = box.x + 20;
await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: x0, y }] });
for (let i = 1; i <= 8; i++) await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: x0 + (box.width * 0.8 * i) / 8, y }] });
await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
await p.waitForTimeout(1500);
const sw = wire.filter((w) => w.route === "activation.resolve");
ok(sw.length === 1 && sw[0].body.action_id === "pin" && sw[0].body.dispatch === true, "a full swipe is one exchange: resolve with dispatch:true for the first leading action", JSON.stringify(sw.map((w) => [w.body.action_id, w.body.dispatch])));
ok(wire.filter((w) => w.route === "activation.dispatch").length === 0, "and no separate dispatch, no sheet");
if (shot) await p.screenshot({ path: `${shot}-swipe.png` });
const undoBtn = p.getByRole("button", { name: /undo/i }).first();
const hasUndo = await undoBtn.waitFor({ timeout: 4000 }).then(() => true, () => false);
ok(hasUndo, "the outcome offers Undo");
if (hasUndo) { await undoBtn.click(); await p.waitForTimeout(1200); ok(wire.some((w) => w.route === "undo" && w.body.type === "undo" && w.body.undo_ref), "Undo goes through the host's undo route with its ref"); }

// ---- a short swipe does nothing
const n0 = wire.length;
await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: x0, y }] });
for (let i = 1; i <= 4; i++) await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: x0 + (box.width * 0.2 * i) / 4, y }] });
await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
await p.waitForTimeout(600);
ok(wire.length === n0, "a short swipe sends nothing");

// ---- Move to collection: choice, then (if the host challenges) the protected confirmation
const openMenu = async (name) => {
  await row.hover(); await row.locator("button.more").click({ force: true });
  await p.getByRole("menuitem", { name }).click();
};
wire.length = 0; replies.length = 0;
await openMenu(/move to collection/i);
await p.waitForSelector("[data-dialog=choose]", { timeout: 8000 });
const labels = await p.locator("[data-dialog=choose] [role=option]").allTextContents();
ok(labels.length > 0, "the picker shows the host's option labels", labels.join(" | "));
if (shot) await p.screenshot({ path: `${shot}-choose.png` });
await p.locator("[data-dialog=choose] [role=option]").first().click();
await p.waitForTimeout(1200);
const resolve = wire.find((w) => w.route === "activation.resolve");
ok(wire[0]?.route === "choice.options" && wire[0].body.type === "choice_options" && !!wire[0].body.activation_context, "choice options are fetched first with the activation context and the slot", wire[0]?.body?.slot);
ok(!!resolve && Object.keys(resolve.body.interaction ?? {}).join() === "choices" && typeof resolve.body.interaction.choices.collection === "string", "the resolve carries only the picked ChoiceRef");
const challenged = !!replies[0]?.confirmation_challenge;
const sheet = await p.locator("[data-dialog=confirm]").isVisible().catch(() => false);
ok(sheet === challenged, "the protected confirmation sheet appears exactly when the host challenged", `challenge ${challenged}, sheet ${sheet}`);
if (sheet) {
  const mat = await p.locator("[data-material] dt").allTextContents();
  ok(mat.length === (replies[0].material ?? []).length && mat.length > 0, "it shows the host's material and nothing else", mat.join(","));
  if (shot) await p.screenshot({ path: `${shot}-confirm.png` });
  await p.getByRole("button", { name: "Cancel" }).click(); await p.waitForTimeout(500);
  ok(!wire.some((w) => w.route === "activation.confirm" || w.route === "activation.dispatch"), "Cancel sends nothing further");
  // again, and confirm this time
  wire.length = 0; replies.length = 0;
  await openMenu(/move to collection/i); await p.waitForSelector("[data-dialog=choose]");
  await p.locator("[data-dialog=choose] [role=option]").first().click();
  await p.waitForSelector("[data-dialog=confirm]", { timeout: 8000 });
  await p.getByRole("button", { name: "Confirm" }).click(); await p.waitForTimeout(1500);
  const routes = wire.map((w) => w.route).join(",");
  ok(routes === "choice.options,activation.resolve,activation.confirm,activation.dispatch", "confirm sends invoke_confirm, then the dispatch with the proof (three exchanges)", routes);
  ok(!!wire.find((w) => w.route === "activation.dispatch")?.body.confirmation_proof, "the dispatch carries the confirmation proof");
} else {
  ok(wire.some((w) => w.route === "activation.dispatch"), "without a challenge it resolves then dispatches, with no sheet", wire.map((w) => w.route).join(","));
}

// ---- Rename: the prompt sheet
wire.length = 0;
await openMenu(/rename/i);
await p.waitForSelector("[data-dialog=prompt]", { timeout: 8000 });
await p.locator("[data-dialog=prompt] input").fill("Renamed by the demo suite");
if (shot) await p.screenshot({ path: `${shot}-prompt.png` });
await p.getByRole("button", { name: "OK" }).click(); await p.waitForTimeout(1500);
const ren = wire.find((w) => w.route === "activation.resolve");
ok(ren?.body.interaction?.prompt?.title === "Renamed by the demo suite", "the prompt answer goes in interaction.prompt under the input's name", JSON.stringify(ren?.body.interaction));
await browser.close(); stop();
process.exit(bad ? 1 : 0);
