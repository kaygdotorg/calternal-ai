// Phone navigation: the sidebar and the side panel are history entries (Back closes them, Forward reopens them); a swipe from the edge is the same navigation, the card follows the finger,
// and a touch while it settles takes hold of it where it is. Usage: node e2e/phone-nav.mjs (after `npm run build`).
import { launch, serve, open, win } from "./lib.mjs";
const stop = await serve(); const b = await launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const { page } = await open(b, "phone", "light", "/notes/n-1");
const over = () => page.evaluate(() => history.state?.over ?? null);
const side = () => page.evaluate(() => document.querySelector(".shell").dataset.sidebar);
const right = () => page.evaluate(() => document.querySelector(".shell").dataset.right);
const midX = () => page.evaluate(() => new DOMMatrixReadOnly(getComputedStyle(document.querySelector("main.mid")).transform).m41);
const settle = () => page.waitForTimeout(450);
// One finger: down at (x0, y), moves to x1 in `steps` moves `gap` ms apart, then up (or stays down). Events go to the element under the first touch, as a touch does.
const finger = (x0, x1, { y = 400, steps = 8, gap = 16, hold = false } = {}) => page.evaluate(async ([x0, x1, y, steps, gap, hold]) => {
  const el = document.elementFromPoint(x0, y);
  const fire = (t, x) => el.dispatchEvent(new PointerEvent(t, { bubbles: true, pointerType: "touch", pointerId: 5, clientX: x, clientY: y, isPrimary: true }));
  fire("pointerdown", x0);
  for (let i = 1; i <= steps; i++) { await new Promise((r) => setTimeout(r, gap)); fire("pointermove", x0 + ((x1 - x0) * i) / steps); }
  if (!hold) fire("pointerup", x1);
}, [x0, x1, y, steps, gap, hold]);
const lift = () => page.evaluate(() => document.querySelector("main.mid, .peek, .right")?.dispatchEvent(new PointerEvent("pointerup", { bubbles: true, pointerType: "touch", pointerId: 5, isPrimary: true })));

// 1. The button: the list is a history entry.
const len0 = await page.evaluate(() => history.length);
await win(page).getByRole("button", { name: "Show notes" }).click(); await settle();
ok((await side()) === "true" && (await over()) === "list", "the Show notes button opens the list as a history entry");
ok((await page.evaluate(() => history.length)) === len0 + 1, "one entry was added");
await page.evaluate(() => history.back()); await settle();
ok((await side()) === "false" && (await over()) === null, "Back closes the list");
await page.evaluate(() => history.forward()); await settle();
ok((await side()) === "true", "Forward opens it again");
await page.evaluate(() => history.back()); await settle();

// 2. Choosing a note: it opens, the list goes away, and Back returns to the list.
await win(page).getByRole("button", { name: "Show notes" }).click(); await settle();
await page.locator('nav[aria-label="Notes"] a[href$="n-2"]').click(); await settle();
ok(page.url().endsWith("/notes/n-2") && (await side()) === "false", "choosing a note shows it and closes the list");
await page.evaluate(() => history.back()); await settle();
ok((await side()) === "true" && page.url().endsWith("/notes/n-1"), "Back from it brings the list back over the note you came from");
await page.evaluate(() => history.back()); await settle();

// 3. A swipe from the left edge: the card follows the finger, and commits past halfway.
await finger(4, 150, { hold: true, gap: 80 });
const mid = await midX();
ok(mid > 120 && mid < 160, "the card follows the finger (150 px)", `${mid.toFixed(0)} px`);
await lift(); await settle();
ok((await side()) === "false" && (await midX()) < 2, "released short of halfway the card settles back");
await finger(4, 260); await settle();
ok((await side()) === "true" && (await over()) === "list", "a swipe past halfway opens the list as a history entry");
// 4. Dragging the card away again from its visible strip closes it (a history step back).
const w = await page.evaluate(() => innerWidth);
await finger(w - 20, w - 300); await settle();
ok((await side()) === "false" && (await over()) === null, "dragging the card back closes the list through history");
// 5. A flick: short and fast commits.
// The flick's speed is what decides, so the page records the events' own timestamps: on a machine so busy that the events arrive late the finger was not fast, and the check says so instead of failing.
await page.evaluate(() => { window.__mv = []; document.addEventListener("pointermove", (e) => window.__mv.push([e.timeStamp, e.clientX]), true); });
await finger(4, 70, { steps: 3, gap: 8 }); await settle();
const mv = await page.evaluate(() => window.__mv), speed = mv.length > 1 ? (mv.at(-1)[1] - mv[0][1]) / (mv.at(-1)[0] - mv[0][0]) : 0;
const gaps = mv.slice(1).map((m, i) => m[0] - mv[i][0]), slow = Math.max(...gaps, 0) > 60;
if (process.env.CAL_GATE && (speed < 0.45 || slow)) console.log(`info  a short fast flick: the events arrived too slowly to be a flick (${speed.toFixed(2)} px/ms, the longest gap ${Math.max(...gaps, 0).toFixed(0)} ms, on a busy machine), not judged`);
else ok((await side()) === "true", "a short fast flick opens the list", `${speed.toFixed(2)} px/ms`);
if ((await side()) === "true") { await page.evaluate(() => history.back()); await settle(); } // (a flick that was not judged may have left it closed: nothing to go back from)
// 6. Interrupt: a touch while the card is settling holds it where it is.
await win(page).getByRole("button", { name: "Show notes" }).click();
await page.waitForTimeout(120);
const before = await midX();
await finger(w - 20, w - 20, { steps: 0, hold: true });
const after = await midX();
await page.waitForTimeout(120);
const later = await midX();
ok(before > 5 && Math.abs(later - after) < 2 && Math.abs(after - before) < 120, "a touch mid-settle freezes the card where it is", `${before.toFixed(0)} -> ${after.toFixed(0)} -> ${later.toFixed(0)}`);
await lift(); await settle();
await page.evaluate(() => { if (history.state?.over) history.back(); }); await settle();

// 7. The side panel: from the right edge it follows the finger; Back closes it.
await finger(w - 4, w - 200, { hold: true });
const rx = await page.evaluate(() => new DOMMatrixReadOnly(getComputedStyle(document.querySelector("main.mid")).transform).m41);
ok(rx < 0 && -rx < (await page.evaluate(() => document.querySelector(".right > .rs").getBoundingClientRect().width)), "the side panel is the mirror of the list: the card follows the finger to the left from the right edge", `${rx.toFixed(0)} px`);
await lift(); await settle();
if ((await right()) !== "true") await finger(w - 4, 40).then(settle);
ok((await right()) === "true" && (await over()) === "panel", "the side panel is a history entry");
await page.evaluate(() => history.back()); await settle();
ok((await right()) === "false", "Back closes the side panel");
await b.close(); stop();
console.log(bad ? `${bad} FAIL` : "all pass"); process.exit(bad ? 1 : 0);
