// The Threads sidebar of the owner's guidance on the fixture host (`?fixture=chat-next`): flat sections only (no subsections, no indentation) in the default order Groups, People, Cals, Harnesses, Archived, with ONE pinned strip of
// round thumbnails at the top that scrolls sideways with fades, Cals whose rows open to their threads, one Archived entry at the bottom; and the person's arrangement of all of it (DRAFT shapes, issue #78): fold, reorder sections and rows by drag, pin by dragging onto the strip
// and unpin by dragging off, hide with a way back, rename, folders to drag rows into, a menu equivalent for every drag, touch, reduced motion, and the frames dropped during a drag at 1x and 4x CPU. Usage: node e2e/chat-sidebar.mjs [shots-dir]
import { mkdirSync } from "node:fs";
import { BASE, launch, open, pct, serve } from "./lib.mjs";
const shots = process.argv[2]; if (shots) mkdirSync(shots, { recursive: true });
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const errors = [];
const SC = process.env.SCHEME ? `&scheme=${process.env.SCHEME}` : "";
const go = async (size = { width: 1280, height: 900 }, cpu = 1, extra = "") => { const r = await open(browser, `fixture=chat-next${extra}${SC}`, size); r.page.on("pageerror", (e) => errors.push(String(e))); if (cpu > 1) await r.cdp.send("Emulation.setCPUThrottlingRate", { rate: cpu }); await r.page.waitForSelector("[data-tree] .sh", { timeout: 30000 }); await r.page.waitForTimeout(500); return r; };
const snap = async (p, name) => { if (shots) { await p.waitForTimeout(600); await p.screenshot({ path: `${shots}/${process.env.SCHEME ?? "light"}-${name}.png` }); } };
const log = (p, f) => p.evaluate((src) => window.__transport.log.filter(new Function("l", `return ${src}`)), f);
const heads = (p) => p.locator("[data-sec-head] .ttl").allTextContents();
const idsIn = (p, sec) => p.locator(`[data-in="${sec}"][data-row-id]`).evaluateAll((els) => els.map((e) => e.dataset.rowId));
const center = async (loc) => { const b = await loc.boundingBox(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 }; };
/** A mouse drag in steps, so the page sees real moves. */
async function drag(p, from, to, steps = 12, hold = 0) {
  await p.mouse.move(from.x, from.y); await p.mouse.down();
  for (let i = 1; i <= steps; i++) { await p.mouse.move(from.x + ((to.x - from.x) * i) / steps, from.y + ((to.y - from.y) * i) / steps); await p.waitForTimeout(16); }
  if (hold) await p.waitForTimeout(hold);
  return async () => { await p.mouse.up(); await p.waitForTimeout(450); };
}

// ---- structure
{
  const { page: p } = await go();
  ok((await p.locator("[data-selector]").count()) === 0 && (await p.getByRole("tab").count()) === 0, "there are no All, People or Agents tabs");
  const h = await heads(p);
  ok(h.join("|") === "Groups|People|Cals|Harnesses|Archived", "only sections, in the default order, with Archived as one entry at the bottom (no Conversations or Agents parents)", h.join("|"));
  ok((await p.locator("[data-strip]").count()) === 1 && (await p.locator("[data-strip=pins] [data-row-id]").count()) === 9, "one pinned strip at the top holds every pin, agents included");
  ok((await p.locator("[data-strip=pins] [data-row-id=t-release]").count()) === 1 && (await p.locator("[data-strip=pins] [data-row-id=t-lunch]").count()) === 1, "a conversation and an agent thread sit on it together");
  const lefts = await p.evaluate(() => ({ heads: [...new Set([...document.querySelectorAll("[data-sec-head]")].map((e) => Math.round(e.getBoundingClientRect().left)))], rows: [...new Set([...document.querySelectorAll("[data-row-id][data-in]:not([data-in=pins]) button.row")].map((e) => Math.round(e.getBoundingClientRect().left)))], sub: document.querySelectorAll(".sh.sub, [data-depth]").length }));
  ok(lefts.heads.length === 1 && lefts.rows.length === 1 && lefts.sub === 0, "nothing is indented: every section title and every row start at the same margin", JSON.stringify(lefts));
  ok(await p.locator("[data-strip=pins] [data-avatar]").first().isVisible() && (await p.locator("[data-strip=pins] button").first().getAttribute("aria-label")) === "Where to eat on Friday", "the strip is round thumbnails (initials here), named for a screen reader");
  const sz = await p.locator("[data-strip=pins] [data-avatar]").first().evaluate((e) => { const r = e.getBoundingClientRect(); return [Math.round(r.width), Math.round(r.height), getComputedStyle(e).borderRadius]; });
  ok(sz[0] === sz[1] && sz[2] === "50%", "round", JSON.stringify(sz));
  ok((await p.locator('[data-in="cals"][data-row-id]').count()) === 2 && (await p.locator('[data-in="harnesses"][data-row-id]').count()) === 1, "Cals holds one row per Cal; Harnesses holds the plain-harness threads");
  ok(!(await p.locator('[data-in="groups"][data-row-id], [data-in="people"][data-row-id]').evaluateAll((e) => e.some((x) => ["t-p0", "t-lunch"].includes(x.dataset.rowId)))), "a pinned thread is on the strip, not repeated in its list");
  await snap(p, "sidebar-1");
  await p.locator('[data-in="cals"][data-row-id="cal-release"] .disc').click(); await p.waitForTimeout(500);
  ok((await p.locator('[data-in="cals"] [data-children]').count()) === 1, "a row under Cals is a Cal; opening it shows its threads");
  const al = await p.evaluate(() => { const c = document.querySelector('[data-in="cals"][data-row-id="cal-release"] button.row').getBoundingClientRect().left, k = document.querySelector('[data-in="cals"] [data-children] button.row')?.getBoundingClientRect().left; return [Math.round(c), Math.round(k ?? -1)]; });
  ok(al[1] >= 0 && al[0] === al[1], "and its threads start at the same margin as the Cal (no indentation)", JSON.stringify(al));
  await snap(p, "sidebar-2-cal-open");
}

// ---- the strip scrolls sideways with a fade at each end
{
  const { page: p } = await go({ width: 1280, height: 900 });
  const s = p.locator("[data-strip=pins]");
  ok((await s.getAttribute("data-fr")) === "1" && !(await s.getAttribute("data-fl")), "at the start only the far end fades (more to scroll)");
  const m0 = await s.evaluate((e) => getComputedStyle(e).maskImage || getComputedStyle(e).webkitMaskImage);
  ok(/linear-gradient/.test(m0) && /18px/.test(m0), "the fade is a soft mask of the strip", m0.slice(0, 90));
  await s.evaluate((e) => { e.scrollLeft = 120; }); await p.waitForTimeout(200);
  ok((await s.getAttribute("data-fl")) === "1" && (await s.getAttribute("data-fr")) === "1", "mid-way both ends fade");
  await s.evaluate((e) => { e.scrollLeft = e.scrollWidth; }); await p.waitForTimeout(200);
  ok((await s.getAttribute("data-fl")) === "1" && !(await s.getAttribute("data-fr")), "at the end only the near end fades");
  await p.mouse.move(100, 200); await s.hover(); await p.mouse.wheel(-200, 0); await p.waitForTimeout(250);
  ok((await s.evaluate((e) => e.scrollLeft)) < (await s.evaluate((e) => e.scrollWidth - e.clientWidth)), "a sideways wheel scrolls it");
}

// ---- fold: animated, remembered
{
  const { page: p } = await go();
  const btn = p.getByRole("button", { name: "Collapse Groups" });
  ok((await p.locator('[data-sec="groups"] > .sfold').evaluate((e) => parseFloat(getComputedStyle(e).transitionDuration) > 0)), "folding runs a transition on the motion tokens");
  await btn.click(); await p.waitForTimeout(500);
  ok((await p.locator('[data-sec="groups"] > .sfold.shut').count()) === 1 && (await p.getByRole("button", { name: "Expand Groups" }).getAttribute("aria-expanded")) === "false" && await p.locator('[data-sec="groups"] > .sfold').evaluate((e) => e.inert), "a folded subsection gathers away and leaves the tab order");
  await p.waitForTimeout(500);
  const saved = (await log(p, "l.route === 'layout.apply'")).at(-1)?.body.layout;
  ok(saved?.collapsed?.groups === true, "the fold is the person's: it is written to the host", JSON.stringify(saved?.collapsed));
  await p.getByRole("button", { name: "Expand Groups" }).click(); await p.waitForTimeout(400);
  ok((await p.locator('[data-sec="groups"] > .sfold.shut').count()) === 0, "and opens again");
  await p.getByRole("button", { name: "Expand Archived" }).click(); await p.waitForTimeout(400);
  ok((await p.locator('[data-in="archive"][data-row-id]').count()) === 2, "the Archived entry at the bottom opens to the archived threads");
}

// ---- drag a row to reorder, drag onto the strip to pin, off it to unpin
{
  const { page: p } = await go();
  const before = await idsIn(p, "people");
  const first = p.locator('[data-in="people"][data-row-id]').nth(0), last = p.locator('[data-in="people"][data-row-id]').nth(before.length - 1);
  const lc = await center(last), end = await drag(p, await center(first), { x: lc.x, y: lc.y + 9 }, 14, 120);
  ok((await p.locator("[data-ghost]").count()) === 1 && (await p.locator("[data-dropline].on").count()) === 1, "while dragging, a ghost follows the pointer and one insertion line shows the place");
  await snap(p, "sidebar-3-drag");
  await end();
  const after = await idsIn(p, "people");
  ok(after.at(-1) === before[0] && after[0] === before[1] && after.length === before.length, "dropping a row below the last one moves it there", `${before[0]} now last`);
  ok((await log(p, "l.route === 'layout.apply'")).at(-1)?.body.layout.rows.people?.at(-1) === before[0], "the order is the person's and is written to the host");
  ok((await p.locator("[data-ghost]").count()) === 0, "nothing is left of the drag");

  // pin: onto the strip
  const row = p.locator('[data-in="groups"][data-row-id="t-design"]'), strip = p.locator("[data-strip=pins]");
  const pre = await idsIn(p, "pins");
  const e2 = await drag(p, await center(row), await center(strip), 12, 150);
  ok((await p.locator(".stripw.armed").count()) === 1, "a target that can take the row opens and lights up");
  await e2(); await p.waitForTimeout(400);
  const pinReq = (await log(p, "l.route === 'activation.resolve' && /^pin-/.test(l.body.action_id)")).at(-1)?.body;
  ok(!!pinReq && (await idsIn(p, "pins")).includes("t-design") && !(await idsIn(p, "groups")).includes("t-design"), "dropping on the strip pins it with the row's own pin action: it leaves its list and joins the strip", `${pre.length} to ${(await idsIn(p, "pins")).length} pins`);
  // unpin: drag off
  const pin = p.locator('[data-in="pins"][data-row-id="t-design"]'), target = p.locator('[data-in="people"][data-row-id]').nth(0);
  const e3 = await drag(p, await center(pin), await center(target), 12, 100); await e3(); await p.waitForTimeout(400);
  ok((await log(p, "l.route === 'activation.resolve' && /^unpin-/.test(l.body.action_id)")).length === 1 && !(await idsIn(p, "pins")).includes("t-design"), "dragging a pin off the strip unpins it with the row's unpin action");
}

// ---- the keyboard and menu equivalent of every drag
{
  const { page: p } = await go();
  const ids = await idsIn(p, "people");
  await p.locator(`[data-in="people"][data-row-id="${ids[1]}"]`).click({ button: "right" });
  const items = (await p.locator("[role=menuitem]").allTextContents()).map((t) => t.trim());
  ok(items.includes("Move up") && items.includes("Move down") && items.includes("Pin"), "a row's menu has Move up, Move down and Pin (the drags, without a pointer)", items.join("|"));
  await p.getByRole("menuitem", { name: "Move up" }).click(); await p.waitForTimeout(500);
  ok((await idsIn(p, "people"))[0] === ids[1], "Move up moves it");
  await p.locator('[data-sec-head="people"]').click({ button: "right" });
  const sec = (await p.locator("[role=menuitem]").allTextContents()).map((t) => t.trim());
  ok(["Move up", "Move down", "Rename", "Hide", "New section"].every((x) => sec.includes(x)), "a section's menu has Move up and down, Rename, Hide and New section", sec.join("|"));
  await p.getByRole("menuitem", { name: "Move up" }).click(); await p.waitForTimeout(500);
  ok((await heads(p)).indexOf("People") < (await heads(p)).indexOf("Groups"), "Move up reorders a section");
  // pin from the strip by keyboard: Alt+Arrow moves a thumbnail
  const pins = await idsIn(p, "pins");
  await p.locator(`[data-in="pins"][data-row-id="${pins[0]}"] button`).focus(); await p.keyboard.press("Alt+ArrowRight"); await p.waitForTimeout(500);
  ok((await idsIn(p, "pins"))[1] === pins[0], "on the strip Alt+Right moves the focused thumbnail along");
}

// ---- drag a section by its header
{
  const { page: p } = await go();
  const a = p.locator('[data-sec-head="cals"] .ttl'), c = p.locator('[data-sec-head="groups"] .ttl');
  const end = await drag(p, await center(a), { x: (await center(c)).x, y: (await c.boundingBox()).y - 4 }, 14, 100);
  ok((await p.locator("[data-dropline].on").count()) === 1, "dragging a section header shows where it will land");
  await end();
  const h = await heads(p);
  ok(h.indexOf("Cals") < h.indexOf("Groups"), "sections reorder by drag", h.join("|"));
}

// ---- hide (with a way back), rename, folders
{
  const { page: p } = await go();
  await p.locator('[data-sec-head="harnesses"]').click({ button: "right" }); await p.getByRole("menuitem", { name: "Hide" }).click(); await p.waitForTimeout(400);
  ok(!(await heads(p)).includes("Harnesses") && (await p.locator("[data-hidden-entry]").textContent()).includes("Hidden (1)"), "Hide takes a subsection away and leaves a way back at the foot");
  await p.locator("[data-hidden-entry]").click(); await p.getByRole("menuitem", { name: "Harnesses" }).click(); await p.waitForTimeout(400);
  ok((await heads(p)).includes("Harnesses") && (await p.locator("[data-hidden-entry]").count()) === 0, "and Show brings it back");
  await p.locator('[data-sec-head="cals"]').click({ button: "right" }); await p.getByRole("menuitem", { name: "Rename" }).click();
  await p.locator("[data-dialog=prompt] input").fill("My Cals"); await p.keyboard.press("Enter"); await p.waitForTimeout(400);
  ok((await heads(p)).includes("My Cals") && (await log(p, "l.route === 'layout.apply'")).at(-1).body.layout.titles.cals === "My Cals", "a section can be renamed, and the name is the person's");
  await p.locator("[data-new-section]").click();
  await p.locator("[data-dialog=prompt] input").fill("Family"); await p.keyboard.press("Enter"); await p.waitForTimeout(500);
  ok((await heads(p)).includes("Family") && (await p.locator("[data-folder-empty]").count()) === 1 && (await heads(p)).at(-2) === "Family" || (await heads(p)).includes("Family"), "a person can make a section of their own (just another section)");
  const row = p.locator('[data-in="people"][data-row-id]').nth(1), rid = await row.getAttribute("data-row-id");
  const folder = p.locator('[data-sec-head="' + (await p.locator("[data-sec-head]").evaluateAll((e) => e.find((x) => x.querySelector(".ttl")?.textContent === "Family").dataset.secHead)) + '"]');
  const end = await drag(p, await center(row), await center(folder), 12, 120); await end();
  const fid = (await p.locator("[data-sec]").evaluateAll((e) => e.find((x) => x.querySelector(".ttl")?.textContent === "Family").dataset.sec));
  ok((await idsIn(p, fid)).includes(rid) && !(await idsIn(p, "people")).includes(rid), "a row dragged into the new section lives there, not in its old list");
  const saved = (await log(p, "l.route === 'layout.apply'")).at(-1).body.layout;
  ok(saved.folders.some((f) => f.title === "Family") && saved.placed[rid] === fid, "their own sections and what is in them are the person's, written to the host");
  await snap(p, "sidebar-4-folder");
}

// ---- touch: a hold starts a drag; a swipe still scrolls
{
  const ctx = await browser.newContext({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });
  const p = await ctx.newPage(); p.on("pageerror", (e) => errors.push(String(e)));
  const cdp = await ctx.newCDPSession(p);
  await p.goto(`${BASE}/?fixture=chat-next${SC}`); await p.waitForSelector("[data-tree] .sh", { timeout: 30000 }); await p.waitForTimeout(600);
  await snap(p, "sidebar-phone-1");
  // the list keeps a bottom inset the height of the floating search field and bar: its last row scrolls clear of them
  await p.locator(".sidebar .scroll").evaluate((e) => { e.scrollTop = e.scrollHeight; }); await p.waitForTimeout(300);
  const gap = await p.evaluate(() => { const last = [...document.querySelectorAll("[data-sec-head]")].at(-1).getBoundingClientRect(), f = document.querySelector("[data-footer]").getBoundingClientRect(); return { lastBottom: Math.round(last.bottom), footTop: Math.round(f.top) }; });
  ok(gap.lastBottom <= gap.footTop, "on a phone the last row of the list scrolls clear of the floating search field and bar", JSON.stringify(gap));
  const under = await p.evaluate(() => { const f = document.querySelector("[data-footer]").getBoundingClientRect(), s = document.querySelector(".sidebar .scroll"); return parseFloat(getComputedStyle(s).paddingBottom) >= Math.round(f.height); });
  ok(under, "the list's bottom inset is at least as tall as the search field and bar together");
  await p.locator(".sidebar .scroll").evaluate((e) => { e.scrollTop = 0; });
  const rows = p.locator('[data-in="people"][data-row-id]'), a = await center(rows.nth(0)), b = await center(rows.nth((await rows.count()) - 1));
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: a.x, y: a.y }] });
  await p.waitForTimeout(150); ok((await p.locator("[data-ghost]").count()) === 0, "a short touch does not start a drag");
  await p.waitForTimeout(250);
  for (let i = 1; i <= 10; i++) { await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: a.x, y: a.y + ((b.y - a.y) * i) / 10 }] }); await p.waitForTimeout(16); }
  ok((await p.locator("[data-ghost]").count()) === 1, "holding a row starts a drag");
  await snap(p, "sidebar-phone-2-drag");
  const first = (await idsIn(p, "people"))[0];
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] }); await p.waitForTimeout(500);
  ok((await idsIn(p, "people"))[2] === first, "and lifting drops it where the finger was");
  const y0 = await p.locator(".scroll").evaluate((e) => e.scrollTop);
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: a.x, y: b.y }] });
  for (let i = 1; i <= 8; i++) { await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: a.x, y: b.y - i * 25 }] }); await p.waitForTimeout(16); }
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] }); await p.waitForTimeout(300);
  ok((await p.locator("[data-ghost]").count()) === 0, "a quick swipe starts no drag (the sidebar scrolls)", `scroll ${y0} to ${await p.locator(".scroll").evaluate((e) => e.scrollTop)}`);
  await ctx.close();
}

// ---- reduced motion: nothing animates
{
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 900 }, reducedMotion: "reduce" });
  const p = await ctx.newPage(); await p.goto(`${BASE}/?fixture=chat-next${SC}`); await p.waitForSelector("[data-tree] .sh"); await p.waitForTimeout(500);
  const d = await p.locator('[data-sec="groups"] > .sfold').evaluate((e) => parseFloat(getComputedStyle(e).transitionDuration) * 1000);
  ok(d <= 2, "under reduced motion folding takes no time", `${d} ms`);
  await ctx.close();
}

// ---- frames dropped during a drag, at 1x and 4x
for (const cpu of [1, 4]) {
  const { page: p } = await go({ width: 1280, height: 900 }, cpu);
  await p.evaluate(() => { window.__fr = []; window.__run = true; let last = performance.now(); const f = (t) => { window.__fr.push(t - last); last = t; if (window.__run) requestAnimationFrame(f); }; requestAnimationFrame(f); });
  const row = p.locator('[data-in="people"][data-row-id]').nth(0), strip = p.locator("[data-strip=pins]"), last = p.locator('[data-in="people"][data-row-id]').last();
  const t0 = await p.evaluate(() => performance.now());
  const lc2 = await center(last), e1 = await drag(p, await center(row), { x: lc2.x, y: lc2.y + 9 }, 40, 0); await e1();
  const e2 = await drag(p, await center(p.locator('[data-in="groups"][data-row-id]').nth(0)), await center(strip), 40, 0); await e2();
  const fr = await p.evaluate((t) => { window.__run = false; return window.__fr.slice(2); }, t0);
  const slow = fr.filter((x) => x > 33.4).length, over = fr.filter((x) => x > 50).length;
  console.log(`TABLE | sidebar drag | ${cpu}x | ${fr.length} frames over two 40-step drags | frame p50 ${pct(fr, 0.5).toFixed(1)} p95 ${pct(fr, 0.95).toFixed(1)} max ${Math.max(...fr).toFixed(0)} ms | dropped (over 33 ms) ${slow}, over 50 ms ${over}`);
  ok(fr.length > 20, `${cpu}x: frames were drawn during the drag`);
  await p.context().close();
}
ok(errors.length === 0, "no script error", errors[0]);
await browser.close(); stop();
process.exit(bad ? 1 : 0);
