// The canvas (docs/ui-shell-status.md, "The rules"), behind ?canvas=1 in the fixtures build. Usage: CAL_URL=... node e2e/canvas.mjs
// niri's structure (a single window fills and is centred, two share, a new one opens beside the focused column, presets, resize between neighbours), the chrome (nothing but the capsule and the sidebar pills; one title pill and one pill of three buttons per window,
// every control with its tooltip), the wallpaper sharp and the wallpaper-derived colour, the old Calternal's frost, input (wheel rule, lock, trackpad never opens a sidebar, two fingers on a phone), motion (pointer animates, keyboard is instant) and frame times.
import { busyNow, judgeTimings, launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const wins = (p) => p.locator("[data-win]");
const stripX = (p) => p.evaluate(() => new DOMMatrixReadOnly(getComputedStyle(document.querySelector(".strip")).transform).m41);
const lum = ([r, g, b]) => { const f = (v) => ((v /= 255) <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4); return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b); };
const rgbOf = (s) => (s.match(/[\d.]+/g) ?? []).slice(0, 3).map(Number);
const ratio = (a, b) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); };
const rects = (p) => p.evaluate(() => [...document.querySelectorAll("[data-win]")].map((w) => { const r = w.getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height, kind: w.dataset.kind }; }));
const canvasBox = (p) => p.locator("[data-canvas]").boundingBox();
/** Whether a column animation was REQUESTED since the last `arm`, and for how long (the page's own calls to `Element.animate` on a column are recorded, so the answer does not depend on when a sample is taken: a 220 ms animation sampled late, or a 0.22 ms one that merely existed for a frame, both used to read as "animating"). Needs `minMs`: the token duration, not a stub. */
const arm = (p) => p.evaluate(() => { if (!window.__animCalls) { window.__animCalls = []; const orig = Element.prototype.animate; Element.prototype.animate = function (k, o) { if (this.hasAttribute?.("data-col")) window.__animCalls.push(typeof o === "number" ? o : o?.duration ?? 0); return orig.call(this, k, o); }; } window.__animCalls.length = 0; });
const animating = async (p, wait = 600, minMs = 150) => { const t = Date.now(); for (;;) { const d = await p.evaluate(() => Math.max(0, ...(window.__animCalls ?? [0]))); if (d >= minMs) return true; if (Date.now() - t > wait) return false; await p.waitForTimeout(25); } };
const item = (p, re) => p.locator("[role^=menuitem]", { hasText: re }).first();
/** Every pixel outside the windows  is the same with and without the windows: two screenshots, the second with the strip hidden, compared outside the windows' boxes. Returns how many pixels differ by more than 3 in a channel. */
async function seam(p) {
  const rs = await p.evaluate(() => [...document.querySelectorAll("[data-win]")].map((w) => w.getBoundingClientRect()).map((r) => [r.left, r.top, r.right, r.bottom]));
  const a = (await p.screenshot()).toString("base64");
  await p.addStyleTag({ content: ".strip { visibility: hidden !important; }" });
  await p.waitForTimeout(200);
  const b = (await p.screenshot()).toString("base64");
  return p.evaluate(async ([a, b, rs]) => {
    const load = (d) => new Promise((res) => { const i = new Image(); i.onload = () => res(i); i.src = "data:image/png;base64," + d; });
    const [ia, ib] = await Promise.all([load(a), load(b)]);
    const draw = (i) => { const c = document.createElement("canvas"); c.width = i.width; c.height = i.height; const x = c.getContext("2d"); x.drawImage(i, 0, 0); return x.getImageData(0, 0, i.width, i.height); };
    const da = draw(ia), db = draw(ib); const k = ia.width / innerWidth;
    let bad = 0;
    for (let y = 0; y < da.height; y += 2) for (let x = 0; x < da.width; x += 2) {
      const X = x / k, Y = y / k;
      if (rs.some((r) => X >= r[0] - 2 && X <= r[2] + 2 && Y >= r[1] - 2 && Y <= r[3] + 2)) continue;
      const i = (y * da.width + x) * 4;
      if (Math.abs(da.data[i] - db.data[i]) > 3 || Math.abs(da.data[i + 1] - db.data[i + 1]) > 3 || Math.abs(da.data[i + 2] - db.data[i + 2]) > 3) bad++;
    }
    return bad;
  }, [a, b, rs]);
}
const key = async (p, chord) => { await p.keyboard.press(chord); await p.waitForTimeout(150); };
try {
  { const { page, ctx } = await open(browser, "desktop", "dark", "/threads/t-idle"); ok((await page.locator("[data-canvas]").count()) === 0, "without ?canvas=1 nothing changes (no canvas)"); await ctx.close(); }
  for (const scheme of ["dark", "light"]) {
    console.log(`-- ${scheme}`);
    const { page: p, ctx } = await open(browser, "desktop", scheme, "/threads?canvas=1");
    await p.waitForTimeout(2200);
    ok((await wins(p).count()) === 3, "a fresh visit seeds a thread, another thread and a note", String(await wins(p).count()));
    // ---- chrome ----
    ok((await p.locator("[data-canvas]").getByText(/\d+ windows?/).count()) === 0 && (await p.getByRole("button", { name: "Open another window" }).count()) === 0, "the canvas carries no counter and no plus");
    const g = await p.evaluate(() => { const c = document.querySelector("[data-canvas]").getBoundingClientRect(); const wb = Math.max(...[...document.querySelectorAll("[data-win]")].map((w) => w.getBoundingClientRect().bottom)); const bodyB = Math.max(...[...document.querySelectorAll("[data-win] > .body")].map((w) => w.getBoundingClientRect().bottom)); return { cb: c.bottom, wb, bodyB, capsule: !!document.querySelector(".mid nav.switcher, .appbar") }; });
    ok(Math.abs(g.wb - g.cb) < 1 && Math.abs(g.bodyB - g.wb) < 1 && !g.capsule, "the windows and their content run to the bottom edge; the app switcher is in the sidebar, not over them", JSON.stringify(g));
    for (const [i, w] of (await wins(p).all()).entries()) {
      const titles = await w.evaluate((e) => [...e.querySelectorAll(".wt")].filter((t) => t.getClientRects().length).length);
      const buttons = await w.locator(".tr button").evaluateAll((b) => b.map((x) => x.getAttribute("aria-label")));
      ok(titles === 1 && /^Full screen\|Share\|(Thread actions|Note actions|More)$/.test(buttons.join("|")), `window ${i + 1}: one title pill and one pill of exactly three buttons`, `${titles} / ${buttons.join(",")}`);
    }
    // tooltips on every control, with the shortcut on full screen
    await p.waitForTimeout(800);
    const w0 = wins(p).nth(0);
    for (const [name, text] of [["Full screen", /Full screen/], ["Share", /Share/], ["Thread actions", /Thread actions|More/]]) {
      await w0.getByRole("button", { name }).hover(); await p.waitForTimeout(1000);
      ok(text.test(await p.locator(".warm-tip").innerText().catch(() => "")), `the ${name} button has its warm tooltip`, await p.locator(".warm-tip").innerText().catch(() => ""));
      await p.mouse.move(5, 5); await p.waitForTimeout(150);
    }
    ok(/Alt|⌥/.test(await (async () => { await w0.getByRole("button", { name: "Full screen" }).hover(); await p.waitForTimeout(700); return p.locator(".warm-tip").innerText().catch(() => ""); })()), "the tooltip of full screen shows its shortcut");
    await p.mouse.move(5, 5);
    // ---- the wallpaper and the colour ----
    ok(!(await p.evaluate(() => !!document.querySelector("[data-canvas]").closest(".mid-card"))) && await p.evaluate(() => { const c = getComputedStyle(document.querySelector("[data-canvas]")); const b = getComputedStyle(document.querySelector("[data-canvas]"), "::before"); return c.backgroundImage === "none" && c.backgroundColor === "rgba(0, 0, 0, 0)" && (b.content === "none" || b.content === "normal"); }), "there is no card round the canvas and the area paints nothing itself (the sidebars' background shows through)");
    ok(await p.evaluate(() => [...document.querySelectorAll("[data-win]")].every((w) => w.dataset.look === "card" && !!w.querySelector(":scope > .pic"))), "every window is the shared middle card (class mid-card) by default: the wallpaper's card copy under the card veil");
    // the looks (all neutral, nothing tinted with the wallpaper) are one preference for every window (canvas/prefs.svelte.ts): Solid is the page paper; Transparent is the paper at a density with no blurred copy and no backdrop filter
    const setLook = async (look) => { await p.evaluate((l) => localStorage.setItem("cal.canvas.prefs", JSON.stringify({ look: l, width: null })), look); await p.reload(); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(2200); };
    await setLook("solid");
    const c = await p.evaluate(() => { const w = document.querySelectorAll("[data-win]")[0]; const probe = document.createElement("i"); probe.style.cssText = "position:absolute;background:var(--paper)"; document.body.append(probe); const paper = getComputedStyle(probe).backgroundColor; probe.remove(); return { bg: getComputedStyle(w).backgroundColor, paper, look: w.dataset.look }; });
    ok(c.look === "solid" && c.bg === c.paper, "Solid is the page paper, a neutral surface (never tinted with the wallpaper)", `${c.bg} / ${c.paper}`);
    await setLook("clear");
    const cl = await p.evaluate(() => { const w = document.querySelectorAll("[data-win]")[0]; return { look: w.dataset.look, pic: !!w.querySelector(":scope > .pic"), f: getComputedStyle(w).backdropFilter, bg: getComputedStyle(w).backgroundColor, d: getComputedStyle(w).getPropertyValue("--fd") }; });
    ok(cl.look === "clear" && !cl.pic && (cl.f === "none" || cl.f === "") && /oklab|color\(|rgba?\(/.test(cl.bg) && +cl.d > 0 && +cl.d <= 1, "Transparent: only a tint over the background (no blurred copy, no backdrop filter), at the least density that reads", JSON.stringify(cl));
    await setLook("card");
    // ---- niri's structure ----
    let r = await rects(p); const cb = await canvasBox(p);
    ok(r.length === 3 && Math.abs(r[1].x - r[0].x - r[0].w - 8) < 1.5 && Math.abs(r[0].w - (cb.width - 24) / 2) < 2, "two columns share the canvas, half each, with an 8 px gap all round and between", `${r[0].w.toFixed(0)} / ${r[1].w.toFixed(0)} of ${cb.width}`);
    ok(Math.abs(r[0].y - cb.y) < 1.5 && Math.abs((r[0].y + r[0].h) - (cb.y + cb.height)) < 1.5 && Math.abs(r[0].x - cb.x - 8) < 1.5, "the windows touch the top and bottom edge of the area as the card does, and keep the card's 8 px inset at the side", JSON.stringify([r[0].x - cb.x, r[0].y - cb.y]));
    ok(Math.abs(r[1].x - r[0].x - r[0].w - 8) < 1.5 && Math.abs(r[2].x - r[1].x) < 1.5 && r[2].y > r[1].y, "the note is stacked below the second thread in its column");
    const diff = await seam(p);
    ok(diff === 0, "the seam: every pixel outside the windows is the same with and without them", `${diff} pixels differ`);
    await p.reload(); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(2000);
    // a window opened by the keyboard: instant, right of the focused column
    await p.locator("[data-win]").nth(0).click({ position: { x: 100, y: 400 } }); await p.waitForTimeout(500);
    const before = (await wins(p).count());
    await arm(p);
    await key(p, "Alt+Shift+KeyN"); await p.waitForTimeout(100);
    ok((await wins(p).count()) === before + 1 && !(await animating(p, 150)), "a keyboard-opened window is instant (no animation)", `${before} -> ${await wins(p).count()}`);
    r = await rects(p);
    const focused = await p.locator("[data-win][data-focused]").evaluate((e) => e.dataset.kind + ":" + [...document.querySelectorAll("[data-win]")].indexOf(e));
    ok((await p.locator("[data-win][data-focused]").getAttribute("data-kind")) === "threads", "the new window has focus", focused);
    // pointer: add beside from the menu animates (a column animation of the token duration is requested)
    await arm(p);
    await p.locator("[data-win][data-focused]").getByRole("button", { name: /^(Thread actions|Note actions|More)$/ }).click();
    await item(p, /Open a thread beside/).click();
    ok(await animating(p), "a pointer-opened window animates (FLIP on the motion tokens)");
    await p.waitForTimeout(700);
    ok(await p.waitForFunction(() => ![...document.querySelectorAll("[data-col]")].some((c) => c.getAnimations().length > 0), null, { timeout: 4000 }).then(() => true, () => false), "the animation ends");
    // ---- keyboard focus moves, presets ----
    const fx = await p.locator("[data-win][data-focused]").evaluate((e) => e.getBoundingClientRect().x);
    await key(p, "Alt+Shift+ArrowLeft"); await p.waitForTimeout(100);
    ok((await p.locator("[data-win][data-focused]").evaluate((e) => e.getBoundingClientRect().x)) < fx, "Alt+Shift+Left focuses the column on the left");
    await arm(p);
    await key(p, "Alt+Shift+KeyR"); await p.waitForTimeout(120);
    ok(!(await animating(p, 150)), "a keyboard width preset is instant");
    const fw = await p.locator("[data-win][data-focused]").evaluate((e) => e.getBoundingClientRect().width);
    ok(Math.abs(fw - ((cb.width - 8) * 2 / 3 - 8)) < 2 || Math.abs(fw - ((cb.width - 8) / 3 - 8)) < 2 || Math.abs(fw - (cb.width - 16)) < 2, "the preset is one of 1/3, 2/3 or full (from half)", fw.toFixed(0));
    // the window menu has no Width and no Look entries (width is the handle, the look a preference)
    await p.locator("[data-win][data-focused]").getByRole("button", { name: /^(Thread actions|Note actions|More)$/ }).click();
    const menuNames = (await p.locator("[role^=menuitem]").allInnerTexts()).map((x) => x.trim());
    ok(!menuNames.some((x) => /^Width |^Look:/.test(x)) && menuNames.some((x) => /Open a thread beside/.test(x)), "the window menu has no Width and no Look entries", menuNames.join(" | ").slice(0, 160));
    await p.keyboard.press("Escape");
    // close back to two windows, then one window fills the canvas and is centred
    while ((await wins(p).count()) > 1) { await key(p, "Alt+Shift+KeyW"); await p.waitForTimeout(100); }
    await p.waitForTimeout(300);
    r = await rects(p);
    ok(r.length === 1 && Math.abs(r[0].w - (cb.width - 16)) < 2 && Math.abs((r[0].x - cb.x) - (cb.x + cb.width - r[0].x - r[0].w)) < 2, "a single window is the card: the area less the card's inset on each side, centred", `${r[0].w.toFixed(0)} of ${cb.width}`);
    const plain = await (async () => { const { page: q, ctx: c2 } = await open(browser, "desktop", scheme, "/threads/t-idle"); await q.waitForTimeout(700); const b = await q.locator(".mid-card").boundingBox(); await c2.close(); return b; })();
    ok(Math.abs(r[0].x - plain.x) < 1 && Math.abs(r[0].y - plain.y) < 1 && Math.abs(r[0].w - plain.width) < 1 && Math.abs(r[0].h - plain.height) < 1, "a single window is identical to the app's own middle card (same box)", `${[r[0].x, r[0].y, r[0].w, r[0].h].map(Math.round)} / ${[plain.x, plain.y, plain.width, plain.height].map(Math.round)}`);
    ok((await wins(p).first().getByRole("button", { name: /Full screen|Restore/ }).count()) === 0 && (await wins(p).first().locator(".tr button").count()) === 2, "a lone window is full already: no full screen control, only Share and More");
    await key(p, "Alt+Shift+KeyN"); await key(p, "Alt+Shift+KeyN");
    r = await rects(p);
    ok(r.length === 3 && Math.abs(r[0].w - r[1].w) < 2 && Math.abs(r[0].w - (cb.width - 24) / 2) < 2, "two windows share it, half each; the third keeps half and the strip scrolls", r.map((x) => x.w.toFixed(0)).join("/"));
    await ctx.close();
  }
  // ---- input rules ----
  {
    const { page: p, ctx } = await open(browser, "desktop", "dark", "/threads?canvas=1");
    await p.waitForTimeout(2200);
    await p.locator("[data-win]").nth(0).click({ position: { x: 100, y: 400 } }); await p.waitForTimeout(400);
    await key(p, "Alt+Shift+KeyN"); await p.waitForTimeout(300);
    await p.mouse.move(5, 5); await p.mouse.wheel(0, -3000); await p.waitForTimeout(300);
    await p.locator("[data-win]").nth(0).click({ position: { x: 100, y: 400 } }); await p.waitForTimeout(500);
    const x0 = await stripX(p);
    const b = await wins(p).nth(1).boundingBox();
    await p.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
    await p.mouse.wheel(0, 300); await p.waitForTimeout(300);
    ok(Math.abs((await stripX(p)) - x0) > 40 || (await stripX(p)) !== x0, "a wheel over an unfocused window moves the canvas", `${x0} -> ${await stripX(p)}`);
    await p.mouse.wheel(0, -2000); await p.waitForTimeout(200);
    const f = await p.locator("[data-win][data-focused]").boundingBox(); const x2 = await stripX(p);
    await p.mouse.move(f.x + f.width / 2, f.y + f.height / 2); await p.mouse.wheel(0, 200); await p.waitForTimeout(300);
    ok(Math.abs((await stripX(p)) - x2) < 1, "a vertical wheel over the focused window never moves the canvas");
    // a trackpad's sideways swipe: pans the canvas when the focused window cannot scroll that way, and never opens or closes a sidebar
    const side0 = await p.locator(".shell").getAttribute("data-sidebar");
    await p.mouse.move(f.x + f.width / 2, f.y + f.height / 2);
    for (let i = 0; i < 8; i++) { await p.mouse.wheel(60, 0); await p.waitForTimeout(16); }
    await p.waitForTimeout(400);
    ok((await p.locator(".shell").getAttribute("data-sidebar")) === side0, "a two-finger sideways swipe never opens or closes a sidebar", `data-sidebar=${side0}`);
    ok((await stripX(p)) !== x2, "it pans the canvas instead (the focused window cannot scroll that way)", `${x2} -> ${await stripX(p)}`);
    await p.mouse.wheel(0, -5000); await p.waitForTimeout(200);
    // the lock
    const w1 = wins(p).nth(1);
    await w1.click({ position: { x: 200, y: 300 } }); await p.waitForTimeout(500);
    const bb = await w1.boundingBox();
    await p.mouse.move(bb.x + bb.width / 2, bb.y + 8); await p.waitForTimeout(300);
    ok((await w1.locator(".lock").evaluate((e) => +getComputedStyle(e).opacity)) > 0.3, "the padlock appears at the window's top middle when the pointer is near");
    await w1.locator(".lock").click();
    ok((await w1.getAttribute("data-locked")) !== null, "the input lock locks the window");
    const xl = await stripX(p);
    const o = await wins(p).nth(0).boundingBox();
    await p.mouse.move(o.x + o.width / 2, o.y + o.height / 2); await p.mouse.wheel(0, 400); await p.waitForTimeout(300);
    ok(Math.abs((await stripX(p)) - xl) < 1, "while locked, the canvas does not move");
    await w1.locator(".lock").click();
    // resize between neighbours
    const w0 = wins(p).nth(0);
    const r0 = await w0.boundingBox(); const cb = await canvasBox(p);
    await p.mouse.move(r0.x + r0.width + 1, r0.y + 200); await p.mouse.down(); await p.mouse.move(r0.x + r0.width + 91, r0.y + 200, { steps: 6 }); await p.mouse.up();
    const r1 = await w0.boundingBox();
    ok(Math.abs(r1.width - (r0.width + 90)) < 4 || Math.abs(r1.width - ((cb.width - 8) * 2 / 3 - 8)) < 2, "dragging the right edge resizes the column (snapping to a preset within 12 px)", `${r0.width.toFixed(0)} -> ${r1.width.toFixed(0)}`);
    // the stacked pair: the note and the window above it (the note's click brings its column into view)
    await p.locator("[data-win][data-kind=notes]").first().click({ position: { x: 200, y: 300 } }); await p.waitForTimeout(700);
    const pair = async () => p.evaluate(() => { const all = [...document.querySelectorAll("[data-win]")]; const note = all.find((w) => w.dataset.kind === "notes"); const nb = note.getBoundingClientRect(); const up = all.find((w) => w !== note && Math.abs(w.getBoundingClientRect().x - nb.x) < 2 && w.getBoundingClientRect().y < nb.y); const ub = up.getBoundingClientRect(); return { up: [ub.x, ub.y, ub.width, ub.height], note: [nb.x, nb.y, nb.width, nb.height], i: all.indexOf(up) }; });
    const q0 = await pair();
    await p.mouse.move(q0.up[0] + q0.up[2] / 2, q0.up[1] + q0.up[3] + 1); await p.mouse.down(); await p.mouse.move(q0.up[0] + q0.up[2] / 2, q0.up[1] + q0.up[3] - 59, { steps: 6 }); await p.mouse.up();
    const q1 = await pair();
    ok(q1.up[3] < q0.up[3] - 40 && q1.note[3] > q0.note[3] + 40, "dragging the line between stacked windows moves it between those two", `${q0.up[3].toFixed(0)}/${q0.note[3].toFixed(0)} -> ${q1.up[3].toFixed(0)}/${q1.note[3].toFixed(0)}`);
    // maximise by the pointer animates; restore
    for (let i = 0; i < 8; i++) await key(p, "Alt+Shift+ArrowLeft");
    await p.waitForTimeout(200);
    await arm(p);
    await p.locator("[data-focused]").getByRole("button", { name: "Full screen" }).click();
    ok(await animating(p), "full screen by the pointer animates");
    await p.waitForTimeout(600);
    const cv = await canvasBox(p), mx = await p.locator("[data-win].maximised").boundingBox();
    ok(Math.abs(mx.width - (cv.width - 16)) < 2 && Math.abs(mx.height - cv.height) < 2, "full screen makes the window fill the area (a card with no neighbours)", `${mx.width}x${mx.height} of ${cv.width}x${cv.height}`);
    await arm(p);
    await key(p, "Alt+Shift+KeyF"); await p.waitForTimeout(100);
    ok((await p.locator("[data-win].maximised").count()) === 0 && !(await animating(p, 150)), "the keyboard restores it instantly");
    // focus follows the press, and the address and sidebars follow focus
    const side = await p.locator("aside.side").innerText();
    await p.locator("[data-win][data-kind=notes]").first().click({ position: { x: 200, y: 200 } }); await p.waitForTimeout(600);
    ok((await p.locator("[data-win][data-kind=notes]").first().getAttribute("data-focused")) !== null && /^\/notes\//.test(new URL(p.url()).pathname) && (await p.locator("aside.side").innerText()) !== side, "pressing a window focuses it; the address and the left sidebar follow its app", new URL(p.url()).pathname);
    // deep links: one window per address
    const items = await p.locator("[data-win][data-kind=threads]").evaluateAll((w) => w.map((x) => x.dataset.item));
    const n = await wins(p).count();
    const go = (id) => p.evaluate((i) => { history.pushState(null, "", "/threads/" + i); dispatchEvent(new PopStateEvent("popstate")); }, id);
    await go(items[0]); await p.waitForTimeout(400); await go(items[1]); await p.waitForTimeout(400); await go(items[0]); await p.waitForTimeout(500);
    ok((await wins(p).count()) === n && (await p.locator("[data-win][data-focused]").getAttribute("data-item")) === items[0], "opening an address that has a window focuses it and makes no second one", `${n} -> ${await wins(p).count()}`);
    // the rest rule: no fade strips; at rest the focused window is fully in view (a neighbour may peek, cut by the canvas edge)
    await p.mouse.wheel(0, -5000); await p.waitForTimeout(300);
    ok((await p.locator(".fade").count()) === 0, "there are no edge fade strips: a window is cut cleanly by the canvas edge");
    const e0 = await wins(p).nth(1).boundingBox(); await p.mouse.move(e0.x + e0.width / 2, e0.y + e0.height / 2); await p.mouse.wheel(0, 90); await p.waitForTimeout(900);
    const inView = await p.evaluate(() => { const c = document.querySelector("[data-canvas]").getBoundingClientRect(), f = document.querySelector("[data-win][data-focused]").getBoundingClientRect(); return f.left >= c.left - 1 && f.right <= c.right + 1; });
    ok(inView, "after a gesture the focused window is fully in view (snapped on the motion tokens)");
    const st = await p.evaluate(() => { const fw = document.querySelector("[data-win][data-focused]"), ow = [...document.querySelectorAll("[data-win]")].find((w) => !w.hasAttribute("data-focused")); return { ring: getComputedStyle(fw).boxShadow, dimF: +getComputedStyle(fw.querySelector(".dim")).opacity, dimO: +getComputedStyle(ow.querySelector(".dim")).opacity, rec: +getComputedStyle(ow.querySelector(".tr")).opacity }; });
    ok(st.ring === "none" && st.dimF === 0 && st.dimO === 1 && st.rec < 1, "the focused window has no ring; an inactive one is dimmed by a neutral veil and its controls recede", JSON.stringify(st));
    await wins(p).nth(0).getByRole("button", { name: "Share" }).click(); await p.waitForTimeout(300);
    ok((await item(p, /Copy link/).count()) === 1, "Share opens a menu with Copy link (and the system share where the browser has one)");
    await p.keyboard.press("Escape");
    ok(await p.evaluate(() => document.querySelector("[data-win] .tr button svg path")?.getAttribute("d")?.startsWith("M14 10l6-6")), "the full screen icon is two outward chevrons");
    await ctx.close();
  }
  // ---- the card's wallpaper layer is a picture that stays still behind the moving cards ----
  {
    const { page: p, ctx } = await open(browser, "desktop", "dark", "/threads?canvas=1");
    await p.waitForTimeout(2200);
    await key(p, "Alt+Shift+KeyN"); await key(p, "Alt+Shift+KeyN");
    const origin = () => p.evaluate(() => { const g = document.querySelector(".win > .pic"); const r = g.getBoundingClientRect(); return [Math.round(r.left), Math.round(r.top)]; });
    for (let i = 0; i < 6; i++) await key(p, "Alt+Shift+ArrowLeft");
    await p.waitForTimeout(300);
    const o0 = await origin();
    const wb = await wins(p).nth(1).boundingBox(); await p.mouse.move(wb.x + wb.width / 2, wb.y + wb.height / 2); await p.mouse.wheel(0, 260); await p.waitForTimeout(500);
    const o1 = await origin();
    ok(Math.abs(o0[0]) <= 1 && Math.abs(o0[1]) <= 1 && Math.abs(o1[0]) <= 1 && Math.abs(o1[1]) <= 1, "each card's wallpaper picture stays at the viewport's origin while the strip scrolls (only transforms move)", `${o0} -> ${o1}`);
    ok(await p.evaluate(() => getComputedStyle(document.querySelector(".win")).backdropFilter === "none"), "no window has a live backdrop filter");
    await ctx.close();
  }
  // ---- six windows: frame times at 4x CPU, as cards and with every window transparent ----
  for (const clear of [false, true]) {
    const { page: p, ctx } = await open(browser, "desktop", "dark", "/threads?canvas=1");
    await p.waitForTimeout(2200);
    for (let i = 0; i < 3; i++) await key(p, "Alt+Shift+KeyN");
    if (clear) { await p.evaluate(() => localStorage.setItem("cal.canvas.prefs", JSON.stringify({ look: "clear", width: null }))); await p.reload(); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(2500); }
    const total = await p.evaluate(() => JSON.parse(localStorage.getItem("cal.canvas")).cols.reduce((n, c) => n + c.wins.length, 0));
    await p.mouse.wheel(0, -9999); await p.waitForTimeout(500);
    // Warm: every column is mounted once before the measurement (mounting a thread view is the one cost of a pan, and it is the same for every look).
    for (let i = 0; i < 16; i++) { await p.mouse.wheel(0, 300); await p.waitForTimeout(40); }
    await p.waitForTimeout(500); await p.mouse.wheel(0, -9999); await p.waitForTimeout(500);
    const cdp = await ctx.newCDPSession(p);
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
    const win = (await wins(p).first().boundingBox());
    await p.mouse.move(win.x + 100, win.y + win.height / 2);
    await p.evaluate(() => { window.__f = []; let l = performance.now(); const f = (t) => { window.__f.push(t - l); l = t; window.__r = requestAnimationFrame(f); }; window.__r = requestAnimationFrame(f); });
    await p.locator("[data-win]").nth(1).click({ position: { x: 200, y: 400 }, force: true }).catch(() => {});
    for (let i = 0; i < 120; i++) { await p.mouse.wheel(i < 60 ? 60 : -60, 0); await p.waitForTimeout(16); }
    await p.waitForTimeout(300);
    const dt = await p.evaluate(() => { cancelAnimationFrame(window.__r); return window.__f.slice(5); });
    const s = [...dt].sort((a, b) => a - b); const p95 = s[Math.floor(s.length * 0.95)], max = s.at(-1), late = dt.filter((v) => v > 24).length;
    console.log(`INFO  frame times while panning, 4x CPU, ${total} windows, ${clear ? "all transparent" : "cards"}: n=${dt.length} p50=${s[s.length >> 1].toFixed(1)}ms p95=${p95.toFixed(1)}ms max=${max.toFixed(1)}ms late(>24ms)=${late}`);
    if (!judgeTimings()) console.log(`SKIP  p95 judgement (recorded above, not judged): the machine is busy (${busyNow().why}); the quiet list judges it: npm run e2e:quiet`);
    else ok(p95 < 24, `panning six ${clear ? "transparent " : ""}windows at 4x CPU keeps the p95 frame under 24 ms`, `p95=${p95.toFixed(1)}`);
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: 1 });
    if (!clear) {
      for (let i = 0; i < 4; i++) await key(p, "Alt+Shift+KeyN");
      await p.mouse.wheel(0, -9999); await p.waitForTimeout(500);
      const all = await p.evaluate(() => JSON.parse(localStorage.getItem("cal.canvas")).cols.reduce((n, c) => n + c.wins.length, 0));
      const mounted = await p.evaluate(() => document.querySelectorAll("[data-win]").length);
      ok(mounted < all, "windows far from the screen are not mounted (not laid out, not painted)", `${mounted} of ${all} windows in the DOM`);
    }
    await ctx.close();
  }
  // ---- a phone: one window per screen; two fingers move the canvas; one finger swipes the sidebar ----
  {
    const { page: p, ctx } = await open(browser, "phone", "dark", "/threads?canvas=1");
    await p.waitForTimeout(2200);
    const cdp = await ctx.newCDPSession(p);
    const touch = (type, pts) => cdp.send("Input.dispatchTouchEvent", { type, touchPoints: pts.map(([x, y], id) => ({ x, y, id })) });
    const w = (await wins(p).first().boundingBox());
    ok(Math.abs(w.width - 390) < 1, "a phone window is the screen's full width, edge to edge as the card is", String(w.width));
    const x0 = await stripX(p);
    await touch("touchStart", [[150, 400], [250, 400]]);
    for (let i = 1; i <= 8; i++) await touch("touchMove", [[150 - i * 20, 400], [250 - i * 20, 400]]);
    await touch("touchEnd", []);
    await p.waitForTimeout(600);
    ok(x0 - (await stripX(p)) > 80, "two fingers moving left bring the next window in", `${x0} -> ${await stripX(p)}`);
    // Where the one-finger swipe starts: a spot that belongs to no control. The strip coasts to a different offset each run, so a fixed point sometimes lands on a window's resize handle (`.rz`) or a message row, whose own handlers
    // stop the event from reaching the shell (Svelte delegates pointer events to the root: a handler that stops propagation keeps the sidebar pull from ever seeing the touch). That is the intended rule (a control keeps its own gesture), so the test must not depend on luck.
    const start = await p.evaluate(() => { for (let x = 40; x <= 140; x += 6) for (let y = 260; y <= 380; y += 20) { const e = document.elementFromPoint(x, y); if (e && !e.closest(".rz, .msg, button, a, input, textarea, [data-win] header, .tr")) return [x, y]; } return [60, 300]; });
    await touch("touchStart", [start]);
    for (let i = 1; i <= 10; i++) await touch("touchMove", [[start[0] + i * 20, start[1]]]);
    await touch("touchEnd", []);
    await p.waitForTimeout(700);
    const open1 = await p.locator(".shell").getAttribute("data-sidebar");
    ok(open1 === "true" || (await p.locator(".narrow .mid").evaluate((e) => new DOMMatrixReadOnly(getComputedStyle(e).transform).m41)) > 100, "one finger swiping across the canvas moves the sidebar into view", `data-sidebar=${open1}`);
    await ctx.close();
  }
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);
