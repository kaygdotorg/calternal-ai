// The Threads description in a browser over the fixture host (`?fixture=threads`; testing/threads-host.ts, invented threads and people): the list and its selector, opening a thread draws the committed page of the item view
// and follows it, the composer sends through the harness and placement choices and the protected confirmation with only device-held values on the wire, a phone shows one pane at a time with Back, and the empty, error,
// stale, loading and unavailable states are drawn in plain words. Usage: node e2e/threads.mjs [shots-directory]   (with a directory, also saves captures of every screen at desktop and phone width)
import { mkdirSync } from "node:fs";
import { launch, open, serve } from "./lib.mjs";
const shots = process.argv[2]; if (shots) mkdirSync(shots, { recursive: true });
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const errors = [];
async function go(query, size) {
  const r = await open(browser, `fixture=threads&${query}`, size);
  r.page.on("pageerror", (e) => errors.push(String(e)));
  return r;
}
const snap = async (p, name) => { if (shots) { await p.waitForTimeout(500); await p.screenshot({ path: `${shots}/${name}.png` }); } };
const DESK = { width: 1280, height: 800 }, PHONE = { width: 390, height: 844 };

// ---- desktop: the list
{
  const { page: p } = await go("", DESK);
  await p.waitForSelector("[data-i] button.row", { timeout: 20000 });
  const titles = await p.locator("[data-i] button.row").evaluateAll((els) => els.map((e) => e.textContent.trim()));
  ok(titles.length === 7 && titles[0].startsWith("Where to eat on Friday"), "the sidebar lists the 7 threads, newest first", titles[0]);
  ok(/\b(min|hour|day|yesterday|ago|\d)/i.test(titles[0]), "each row says when it last changed, in relative time", titles[0]);
  ok(await p.getByText("Archived").first().isVisible(), "the selector offers Archived");
  ok((await p.locator("[data-count-of=archived]").textContent()) === "2", "the Archived option carries its exact count (from its count bind)");
  ok(/Archived\s+2/.test(await p.locator("[data-selector=view]").innerText()), "and says it where the person looks: \"Archived 2\"", (await p.locator("[data-selector=view]").innerText()).replace(/\s+/g, " "));
  ok(await p.locator("[data-view-state=idle]").isVisible(), "with nothing open the card says so in plain words");
  await snap(p, "desktop-1-list");

  // ---- open a thread
  await p.locator("[data-i] button.row").first().click();
  await p.waitForSelector("[data-snapshot=heading]", { timeout: 10000 });
  ok((await p.locator("[data-snapshot=heading]").count()) === 6, "opening a thread draws its committed page: one heading per message");
  ok((await p.locator("[data-snapshot=heading] time[datetime]").count()) === 6, "every message carries its UTC instant and shows it in local time");
  ok((await p.locator("[data-snapshot=code]").count()) === 1 && (await p.locator("[data-snapshot=attachment]").count()) === 1, "code and an attachment are drawn by their own nodes");
  ok(await p.locator("main h1").textContent().then((t) => t.includes("Where to eat")), "the header carries the open thread's title");
  const opened = await p.evaluate(() => window.__transport.log.filter((l) => l.route === "item_view.open").map((l) => l.body));
  ok(opened.length >= 1 && opened[0].live === true && opened[0].slot === "body" && opened[0].component === "transcript", "item_view.open is asked live for the body slot of the transcript component", JSON.stringify(opened[0]));
  await snap(p, "desktop-2-thread");
  // follow: a committed message arrives without a reload
  await p.waitForFunction(() => document.querySelectorAll("[data-snapshot=heading]").length > 6, null, { timeout: 15000 }).then(() => ok(true, "a message committed later arrives through item_view.follow without a reload"), () => ok(false, "a message committed later arrives through item_view.follow without a reload"));

  // ---- send: harness, placement, protected confirmation
  const box = p.locator("main textarea").first();
  await box.fill("Book the river place for 19:00, please.");
  await p.getByRole("button", { name: "Send" }).click();
  await p.waitForSelector("[data-dialog=choose]", { timeout: 8000 });
  ok(await p.locator("[data-dialog=choose] h2").textContent().then((t) => t === "Agent"), "Send first asks which agent (the harnesses extension point's labels)");
  await snap(p, "desktop-3-agent-picker");
  await p.locator("[data-dialog=choose] button[role=option]", { hasText: "Codex" }).click();
  await p.waitForSelector("[data-dialog=choose] h2:text('Run on')", { timeout: 8000 });
  ok((await p.locator("[data-dialog=choose] button[data-default]").textContent()).includes("Desk machine"), "then where to run, with the host's eligible default marked");
  await snap(p, "desktop-4-placement-picker");
  await p.locator("[data-dialog=choose] button[role=option]", { hasText: "Laptop" }).click();
  await p.waitForSelector("[data-dialog=confirm]", { timeout: 8000 });
  const mat = await p.locator("[data-material]").textContent();
  ok(/Start an agent run/.test(mat) && /Laptop/.test(mat), "the protected confirmation shows the material the host wrote, including the chosen machine and login", mat.replace(/\s+/g, " "));
  await snap(p, "desktop-5-confirm");
  await p.getByRole("button", { name: "Confirm" }).click();
  await p.waitForFunction(() => document.querySelector("main textarea")?.value === "", null, { timeout: 8000 }).then(() => ok(true, "the draft clears after the host's ok"), () => ok(false, "the draft clears after the host's ok"));
  const wire = await p.evaluate(() => window.__transport.log.filter((l) => l.route.startsWith("activation") || l.route.endsWith("choices") || l.route === "choice.options"));
  const resolve = wire.find((l) => l.route === "activation.resolve" && l.body.action_id === "send")?.body;
  ok(resolve?.interaction?.form?.composer?.message?.startsWith("Book the river") && resolve.interaction.choices.harness === "h-codex" && resolve.interaction.choices.placement === "laptop", "the resolve carries the typed text and the two picked refs", JSON.stringify(resolve?.interaction));
  ok(!JSON.stringify(resolve).match(/"item"|"viewer"|"route"/), "and nothing resolved on the device (no item, viewer or route value)");
  const ext = wire.find((l) => l.route === "extension.choices")?.body, plc = wire.find((l) => l.route === "choice.options")?.body;
  ok(ext?.app === "threads" && ext.point === "harnesses", "the extension choice names the app and the harnesses point", JSON.stringify(ext));
  ok(plc?.harness === "harness-codex" && !!plc.activation_id, "the placement request carries the picked harness's typed ref and the activation id", JSON.stringify(plc));

  // ---- archived selector
  await p.getByRole("tab", { name: /Archived/ }).or(p.getByText("Archived", { exact: false }).first()).first().click().catch(() => {});
}

// ---- desktop: states
for (const [scenario, check, text] of [
  ["empty", "[data-i], .state, [data-state]", /No threads yet/],
  ["error", "body", /Threads could not load/],
]) {
  const { page: p } = await go(`scenario=${scenario}`, DESK);
  await p.waitForFunction((t) => new RegExp(t).test(document.body.innerText), text.source, { timeout: 15000 }).then(() => ok(true, `the ${scenario} state of the list is drawn: ${text.source}`), () => ok(false, `the ${scenario} state of the list is drawn: ${text.source}`));
  await snap(p, `desktop-state-${scenario}`);
}
for (const [scenario, sel, desc] of [["stale", "[data-view-state=stale]", "a retained earlier revision is marked as such"], ["unavailable", "[data-view-state=unavailable]", "an unavailable thread says so"], ["loading", "[data-view-state=loading]", "a slow thread shows a loading shell, not invented content"]]) {
  const { page: p } = await go(`scenario=${scenario}`, DESK);
  await p.waitForSelector("[data-i] button.row", { timeout: 20000 });
  await p.locator("[data-i] button.row").first().click();
  await p.waitForSelector(sel, { timeout: 10000 }).then(() => ok(true, desc), () => ok(false, desc));
  await snap(p, `desktop-state-${scenario}`);
}

// ---- phone: one pane at a time
{
  const { page: p } = await go("", PHONE);
  await p.waitForSelector("[data-i] button.row", { timeout: 20000 });
  ok((await p.locator("main").count()) === 0, "on a phone the list is alone on screen at first");
  await snap(p, "phone-1-list");
  await p.locator("[data-i] button.row").first().click();
  await p.waitForSelector("[data-snapshot=heading]", { timeout: 10000 });
  ok((await p.locator("[data-sidebar]").count()) === 0 && (await p.locator("[data-back]").isVisible()), "opening a thread shows the card alone, with Back");
  const w = await p.evaluate(() => ({ s: document.documentElement.scrollWidth, c: document.documentElement.clientWidth }));
  ok(w.s <= w.c, "nothing overflows the phone's width", JSON.stringify(w));
  const back = await p.locator("[data-back]").boundingBox();
  ok(back.width >= 44 && back.height >= 44, "Back is a 44 pt target");
  await snap(p, "phone-2-thread");
  const pbox = p.locator("main textarea").first();
  await pbox.fill("Sounds good."); await pbox.press("Shift+Enter");
  ok((await pbox.inputValue()) === "Sounds good.\n" && (await p.locator("[data-dialog]").count()) === 0, "Shift+Enter starts a new line and sends nothing");
  await pbox.press("Backspace"); await pbox.press("Enter");
  await p.waitForSelector("[data-dialog=choose]", { timeout: 8000 }).then(() => ok(true, "Enter sends (it asks which agent first)"), () => ok(false, "Enter sends (it asks which agent first)"));
  await snap(p, "phone-3-agent-picker");
  await p.locator("[data-dialog=choose] button[role=option]").first().click();
  await p.waitForSelector("[data-dialog=choose] h2:text('Run on')", { timeout: 8000 });
  await p.locator("[data-dialog=choose] button[role=option]").first().click();
  await p.waitForSelector("[data-dialog=confirm]", { timeout: 8000 });
  await snap(p, "phone-4-confirm");
  await p.getByRole("button", { name: "Cancel" }).click();
  await p.locator("[data-back]").click();
  await p.waitForSelector("[data-sidebar]", { timeout: 5000 }).then(() => ok(true, "Back returns to the list"), () => ok(false, "Back returns to the list"));
}
ok(errors.length === 0, "no script error", errors[0]);
await browser.close(); stop();
process.exit(bad ? 1 : 0);
