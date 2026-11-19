// Bulk Trash against the step 0 host's Notes view, with a multiple-selection list of its `notes` bind added to the view by the page (`extend=multi:notes`; the template's own `trash-selected` header action takes the ticked set): ticking rows sends nothing; the button sends the ticked row ids once to `selection.set`
// (the host's checked set), takes the host-issued `selection_ref` back, and the activation names ONLY that reference (never the ids). A bulk set is material, so the host may challenge: the protected sheet then shows what
// the host wrote. The Notes template declares no multiple-selection list (selectionMode lives on a list node, not on a sidebar section), hence the added list; the home view's authority fixture answers selection.set with 404. Usage: node e2e/bulk.mjs [screenshot-prefix]
import { readFileSync } from "node:fs";
import { launch, open, serve } from "./lib.mjs";
const HOST = process.env.HOST ?? "http://127.0.0.1:43125/step0/v2";
const bearer = readFileSync(process.env.HOST_BEARER_FILE ?? `${process.env.HOME}/Developer/calternal-ai-lanes/.owner.token`, "utf8").trim();
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const { page: p } = await open(browser, `host=${encodeURIComponent(HOST)}&bearer=${encodeURIComponent(bearer)}&viewName=notes&extend=multi:notes`);
const wire = []; p.on("request", (r) => { if (r.method() === "POST" && /selection|activation|choice|undo/.test(r.url())) wire.push({ route: new URL(r.url()).pathname.split("/").pop(), body: r.postDataJSON() }); });
const replies = []; p.on("response", async (r) => { if (/activation\.(resolve|dispatch)|selection\.set/.test(r.url())) replies.push({ route: new URL(r.url()).pathname.split("/").pop(), status: r.status(), body: await r.text().catch(() => "") }); });
await p.waitForSelector("[data-node=list] [data-i] input[type=checkbox]", { timeout: 30000 });
await p.waitForTimeout(800);
const boxes = p.locator("[data-node=list] [data-i] input[type=checkbox]");
// The first row is ticked by keyboard alone: arrow into the row (no context-menu key), the box is named by the row title, Space ticks it.
const first = p.locator("[data-node=list] [data-i]").first(); const firstTitle = (await first.locator(".t").first().innerText()).trim();
await first.locator("button.row").focus(); await p.keyboard.press("ArrowRight");
const focused = await p.evaluate(() => ({ type: document.activeElement?.getAttribute("type"), label: document.activeElement?.getAttribute("aria-label") }));
ok(focused.type === "checkbox" && focused.label === firstTitle, "ArrowRight from a row reaches its tick box, named by the row title (not its id)", JSON.stringify(focused));
await p.keyboard.press("Space");
ok(await boxes.nth(0).isChecked(), "Space on the focused tick box ticks the row");
await boxes.nth(1).check();
ok(wire.length === 0, "ticking rows sends nothing");
const ids = await p.evaluate(() => [...document.querySelectorAll("[data-node=list] [data-i]")].slice(0, 2).map((e) => e.querySelector("input[type=checkbox]").getAttribute("aria-label").replace("Select ", "")));
await p.locator("[data-action=trash-selected]").click();
await p.waitForTimeout(2500);
const set = wire.find((w) => w.route === "selection.set");
ok(set?.body.type === "selection_set" && set.body.bind === "notes" && JSON.stringify([...set.body.rows].sort()) === JSON.stringify([...ids].sort()), "the ticked ids go once to selection.set, as the catalogue's SelectionSetRequest", JSON.stringify(set?.body).slice(0, 140));
const setReply = replies.find((r) => r.route === "selection.set");
ok(setReply?.status === 200 && /selection_ref/.test(setReply.body), "the host answers with a selection_ref", String(setReply?.status));
const resolve = wire.find((w) => w.route === "activation.resolve");
ok(!!resolve && !!resolve.body.interaction?.selection_ref && !JSON.stringify(resolve.body).includes(ids[0]), "the activation names only the host-issued selection_ref, never the row ids", JSON.stringify(resolve?.body?.interaction));
const r = replies.find((x) => x.route === "activation.resolve");
const challenged = /confirmation_challenge/.test(r?.body ?? "");
if (challenged) {
  const sheet = await p.locator("[data-dialog=confirm]").isVisible();
  ok(sheet, "the host challenged a bulk set: the protected sheet shows its material", (await p.locator("[data-material]").innerText().catch(() => "")).replace(/\n/g, " | ").slice(0, 120));
  if (process.argv[2]) await p.screenshot({ path: `${process.argv[2]}-confirm.png` });
  await p.getByRole("button", { name: "Confirm" }).click(); await p.waitForTimeout(1500);
  ok(wire.some((w) => w.route === "activation.confirm") && wire.some((w) => w.route === "activation.dispatch" && w.body.confirmation_proof), "Confirm then dispatch with the proof");
} else {
  console.log(`INFO  host answered the resolve without a challenge: ${(r?.body ?? "").slice(0, 160)}`);
  ok(/"state":"ok"|resolution_ref/.test(r?.body ?? ""), "the bulk resolve was accepted", String(r?.status));
}
const disp = replies.find((x) => x.route === "activation.dispatch");
ok(/"state":"ok"/.test(disp?.body ?? ""), "the dispatch outcome is ok", (disp?.body ?? "").slice(0, 120));
console.log("INFO  wire: " + wire.map((w) => w.route).join(","));
await p.waitForTimeout(1200);
const toast = await p.getByRole("button", { name: /undo/i }).first().isVisible().catch(() => false);
console.log(`INFO  Undo offered after the bulk Trash: ${toast}`);
if (process.argv[2]) await p.screenshot({ path: `${process.argv[2]}-after.png` });
await browser.close(); stop();
process.exit(bad ? 1 : 0);
