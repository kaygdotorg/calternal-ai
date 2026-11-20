// The Threads description the plugin ships today (`plugins/calternal-threads/src/description.ts`, what its manifest declares) in a browser over the fixture host (`?fixture=threads`; invented threads and people): the list with its
// viewer-private marks, opening a thread draws entries and follows them, the composer sends through the host's agent and placement choices and the protected confirmation with only device-held values on the wire,
// a phone shows one pane at a time with Back, and the empty, error, stale, loading and unavailable states are drawn in plain words. Usage: node e2e/threads.mjs [shots-directory]
import { mkdirSync } from "node:fs";
import { launch, open, serve } from "./lib.mjs";
const shots = process.argv[2]; if (shots) mkdirSync(shots, { recursive: true });
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const errors = [];
const go = async (query, size) => { const r = await open(browser, `fixture=threads&${query}`, size); r.page.on("pageerror", (e) => errors.push(String(e))); return r; };
const snap = async (p, name) => { if (shots) { await p.waitForTimeout(500); await p.screenshot({ path: `${shots}/${name}.png` }); } };
const DESK = { width: 1280, height: 800 }, PHONE = { width: 390, height: 844 };
const log = (p, f) => p.evaluate((src) => window.__transport.log.filter(new Function("l", `return ${src}`)), f);
const openFirst = async (p) => { await p.waitForSelector("[data-i] button.row", { timeout: 20000 }); await p.locator("[data-i] button.row").first().evaluate((e) => e.click()); await p.waitForSelector("[data-entry]", { timeout: 10000 }); };

{
  const { page: p } = await go("", DESK);
  await p.waitForSelector("[data-i] button.row", { timeout: 20000 });
  const titles = await p.locator("[data-i] button.row").evaluateAll((els) => els.map((e) => e.textContent.trim()));
  ok(titles.length === 7 && titles[0].startsWith("Where to eat on Friday"), "the sidebar lists the 7 threads, pinned first", titles[0].slice(0, 40));
  ok(/\b(min|hour|day|yesterday|ago|\d)/i.test(titles[0]), "each row says when it last changed, in relative time");
  ok((await p.locator("[data-count-of=archived]").textContent()) === "2" && /Archived\s+2/.test(await p.locator("[data-selector=view]").innerText()), "the Archived option carries its exact count");
  const one = await p.locator("[data-i] button.row").first().evaluate((e) => ({ second: !!e.querySelector(".s"), time: e.querySelector("[data-mark=text]")?.textContent ?? "", h: e.getBoundingClientRect().height }));
  ok(!one.second && /^(now|\d+[mhdw]|yesterday)$/.test(one.time), "a thread row is one line: the title, then the time and the counts at its end", JSON.stringify(one));
  await snap(p, "desktop-1-list");
  await openFirst(p);
  ok((await p.locator("main h1").textContent()).includes("Where to eat"), "opening a thread shows its title in the header");
  ok((await p.locator("[data-entry]").count()) === 9 && (await p.locator("[data-marker=date]").count()) === 2, "and its entries and date lines");
  const opened = (await log(p, "l.route === 'item_view.open'"))[0].body;
  ok(opened.live === true && opened.slot === "body" && opened.component === "transcript" && opened.position === "unread", "item_view.open is asked live for the body slot, at the unread position", JSON.stringify(opened));
  await p.waitForFunction(() => document.querySelectorAll("[data-entry]").length > 9, null, { timeout: 15000 }).then(() => ok(true, "a message committed later arrives through item_view.follow"), () => ok(false, "a message committed later arrives through item_view.follow"));
  await snap(p, "desktop-2-thread");
  // send with chips
  const box = p.locator("main textarea").first();
  await box.fill("Book the river place for 19:00, please.");
  await p.locator("[data-chip=harness]").click();
  await p.waitForSelector("[data-dialog=choose] h2:text('Agent')", { timeout: 8000 });
  await p.locator("[data-dialog=choose] button[role=option]", { hasText: "Codex" }).click();
  await p.waitForFunction(() => /Desk machine/.test(document.querySelector("[data-chip=placement]")?.textContent ?? ""), null, { timeout: 5000 });
  ok(true, "the agent chip takes the pick and Run on shows the host's default");
  await p.locator("[data-chip=placement]").click();
  await p.waitForSelector("[data-dialog=choose] h2:text('Run on')", { timeout: 8000 });
  ok((await p.locator("[data-dialog=choose] button[data-default]").textContent()).includes("Desk machine"), "the placement picker marks the host's eligible default");
  await p.locator("[data-dialog=choose] button[role=option]", { hasText: "Laptop" }).click();
  await box.press("Enter");
  await p.waitForSelector("[data-dialog=confirm]", { timeout: 8000 });
  const mat = await p.locator("[data-material]").textContent();
  ok(/Start an agent run/.test(mat) && /Laptop/.test(mat), "the protected confirmation shows the material the host wrote, including the chosen machine and login");
  await snap(p, "desktop-3-confirm");
  await p.locator("[data-dialog] button", { hasText: "Confirm" }).click();
  await p.waitForFunction(() => document.querySelector("main textarea")?.value === "", null, { timeout: 8000 }).then(() => ok(true, "the draft clears after the host's ok"), () => ok(false, "the draft clears after the host's ok"));
  const resolve = (await log(p, "l.route === 'activation.resolve' && l.body.action_id === 'send'"))[0]?.body;
  ok(resolve?.interaction?.form?.composer?.message?.startsWith("Book the river") && resolve.interaction.choices.harness === "h-codex" && resolve.interaction.choices.placement === "laptop", "the resolve carries the typed text and the two picked refs", JSON.stringify(resolve?.interaction));
  ok(!JSON.stringify(resolve).match(/"item"|"viewer"|"route"/), "and nothing resolved on the device (no item, viewer or route value)");
  const ext = (await log(p, "l.route === 'extension.choices'"))[0]?.body, plc = (await log(p, "l.route === 'choice.options' && l.body.slot === 'placement'")).at(-1)?.body;
  ok(ext?.app === "threads" && ext.point === "harnesses", "the extension choice names the app and the harnesses point");
  ok(plc?.harness === "harness-codex" && !!plc.activation_id, "the placement request carries the picked harness's typed ref and the activation id");
  ok((await log(p, "l.route === 'entry_composer.draft'")).length >= 1, "the draft was kept for the person through the host");
}
for (const [scenario, text] of [["empty", /No threads yet/], ["error", /Threads could not load/]]) {
  const { page: p } = await go(`scenario=${scenario}`, DESK);
  await p.waitForFunction((t) => new RegExp(t).test(document.body.innerText), text.source, { timeout: 15000 }).then(() => ok(true, `the ${scenario} state of the list is drawn: ${text.source}`), () => ok(false, `the ${scenario} state of the list is drawn: ${text.source}`));
  await snap(p, `desktop-state-${scenario}`);
}
for (const [scenario, sel, desc] of [["stale", "[data-view-state=stale]", "a retained earlier revision is marked as such"], ["unavailable", "[data-view-state=unavailable]", "an unavailable thread says so"], ["loading", "[data-view-state=loading]", "a slow thread shows a loading shell, not invented content"], ["lapsed", "[data-access=read_only]", "a thread whose group lapsed is read-only, with its reason"], ["reconnect", "[data-view-state=reconnecting]", "a dropped connection reads Reconnecting, never failed"]]) {
  const { page: p } = await go(`scenario=${scenario}`, DESK);
  await p.waitForSelector("[data-i] button.row", { timeout: 20000 });
  await p.locator("[data-i] button.row").first().evaluate((e) => e.click());
  await p.waitForSelector(sel, { timeout: 12000 }).then(() => ok(true, desc), () => ok(false, desc));
  await snap(p, `desktop-state-${scenario}`);
}
{
  const { page: p } = await go("", PHONE);
  await p.waitForSelector("[data-i] button.row", { timeout: 20000 });
  ok((await p.locator("main").count()) === 0, "on a phone the list is alone on screen at first");
  await snap(p, "phone-1-list");
  await openFirst(p);
  ok((await p.locator("[data-sidebar]").count()) === 0 && (await p.locator("[data-back]").isVisible()), "opening a thread shows the card alone, with Back");
  const w = await p.evaluate(() => ({ s: document.documentElement.scrollWidth, c: document.documentElement.clientWidth }));
  ok(w.s <= w.c, "nothing overflows the phone's width", JSON.stringify(w));
  const back = await p.locator("[data-back]").boundingBox(); ok(back.width >= 44 && back.height >= 44, "Back is a 44 pt target");
  await snap(p, "phone-2-thread");
  const pbox = p.locator("main textarea").first();
  await pbox.fill("Sounds good."); await pbox.press("Shift+Enter");
  ok((await pbox.inputValue()) === "Sounds good.\n" && (await p.locator("[data-dialog]").count()) === 0, "Shift+Enter starts a new line and sends nothing");
  await p.locator("[data-back]").click();
  await p.waitForSelector("[data-sidebar]", { timeout: 5000 }).then(() => ok(true, "Back returns to the list"), () => ok(false, "Back returns to the list"));
}
ok(errors.length === 0, "no script error", errors[0]);
await browser.close(); stop();
process.exit(bad ? 1 : 0);
