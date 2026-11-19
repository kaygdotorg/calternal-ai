// The host components in a browser over the synthetic stand-in (the step 0 host has no routes for them): choosing a thread draws its transcript from `transcript.page` and stamps the first draw (data-thread-rendered,
// calternal:thread-rendered, the calternal:thread:open measure); the composer lists only what `composer.options` returned, holds the draft on the device (nothing crosses until Send), sends once with final_text and
// clears; the request card resolves only through its own buttons. Usage: node e2e/components.mjs [screenshot.png]
import { launch, open, serve } from "./lib.mjs";
const shot = process.argv[2];
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const { page: p } = await open(browser, "mock=40&fixture=threads-app&synthetic=1", { width: 1500, height: 900 });
const errors = []; p.on("pageerror", (e) => errors.push(String(e)));
await p.waitForSelector("[data-i] button.row", { timeout: 20000 });
const src = (fn) => p.evaluate(fn);
await p.locator("[data-i] button.row").nth(2).click();
await p.waitForSelector("[data-event]", { timeout: 10000 });
ok((await p.locator("[data-event]").count()) >= 10, "choosing a thread draws its transcript from the host's committed page");
const stamp = await src(() => ({ id: document.documentElement.dataset.threadRendered, m: performance.getEntriesByName("calternal:thread:open").length }));
ok(!!stamp.id && stamp.m >= 1, "the first screenful stamps data-thread-rendered and records calternal:thread:open", JSON.stringify(stamp));
const atEnd = await p.evaluate(() => { const s = document.querySelector("[data-host-component=turn-transcript] .scroll"); return s.scrollHeight - s.scrollTop - s.clientHeight < 40; });
ok(atEnd, "it opens at the end (follow: end)");
const selects = await p.locator("main select[data-control]").evaluateAll((els) => els.map((e) => e.dataset.control));
ok(selects.join() === "provider,model,reasoning,permission,machine", "the composer lists exactly the controls the host returned options for", selects.join());
const before = await src(() => window.__source.log.filter((l) => l.route.startsWith("activation")).length);
await p.locator("main textarea").first().fill("typed on the device");
await p.waitForTimeout(300);
ok((await src(() => window.__source.log.filter((l) => l.route.startsWith("activation")).length)) === before, "typing sends nothing");
await p.locator("main [data-send]").click(); await p.waitForTimeout(800);
const confirm = await src(() => window.__source.log.find((l) => l.route === "activation.confirm")?.body);
ok(confirm?.final_text === "typed on the device", "Send is one invoke_confirm carrying final_text");
ok((await p.locator("main textarea").first().inputValue()) === "", "the draft cleared after ok");
await p.waitForSelector("[data-host-component=request-card] [data-material]", { timeout: 5000 }).catch(() => {});
const card = await p.locator("[data-host-component=request-card] button").count();
if (card) { await p.getByRole("button", { name: "Deny" }).first().click(); await p.waitForTimeout(500); ok(await src(() => window.__source.log.some((l) => l.route === "request.resolve" && l.body.decision === "deny")), "the request card resolves through its own buttons"); }
else console.log("INFO  no request card drawn for this thread (no request ref)");
ok(errors.length === 0, "no script error", errors[0]);
if (shot) await p.screenshot({ path: shot });
await browser.close(); stop();
process.exit(bad ? 1 : 0);
