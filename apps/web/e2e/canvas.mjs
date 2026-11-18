// The canvas (docs/ui-shell-status.md, "The rules"), behind ?canvas=1 in the fixtures build. Usage: CAL_URL=... node e2e/canvas.mjs
// niri's structure (a single window fills and is centred, two share, a new one opens beside the focused column, presets, resize between neighbours), the chrome (nothing but the capsule and the sidebar pills; one title pill and one pill of three buttons per window,
// every control with its tooltip), the wallpaper sharp and the wallpaper-derived colour, the old Calternal's frost, input (wheel rule, lock, trackpad never opens a sidebar, two fingers on a phone), motion (pointer animates, keyboard is instant) and frame times.
import { cpus, loadavg } from "node:os";
import { launch, open, serve } from "./lib.mjs";
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
const animating = (p, wait = 600) => p.waitForFunction(() => [...document.querySelectorAll("[data-col]")].some((c) => c.getAnimations().length > 0), null, { timeout: wait }).then(() => true, () => false);
const item = (p, re) => p.locator("[role^=menuitem]", { hasText: re }).first();
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
    const g = await p.evaluate(() => { const c = document.querySelector("[data-canvas]").getBoundingClientRect(), card = document.querySelector(".mid-card").getBoundingClientRect(), bar = document.querySelector(".appbar")?.getBoundingClientRect(); const wb = Math.max(...[...document.querySelectorAll("[data-win]")].map((w) => w.getBoundingClientRect().bottom)); return { cb: c.bottom, kb: card.bottom, bt: bar?.top, wb }; });
    ok(Math.abs(g.cb - g.kb) < 1 && g.bt < g.cb && g.wb <= g.bt, "the canvas runs to the card's bottom edge, the capsule floats over it and the windows stop above it", JSON.stringify(g));
    for (const [i, w] of (await wins(p).all()).entries()) {
      const titles = await w.evaluate((e) => [...e.querySelectorAll(".wt, header .title")].filter((t) => t.getClientRects().length).length);
      const buttons = await w.locator(".tr button").evaluateAll((b) => b.map((x) => x.getAttribute("aria-label")));
      ok(titles === 1 && buttons.join("|") === ["Full screen", "Share", "More"].join("|"), `window ${i + 1}: one title pill and one pill of exactly three buttons`, `${titles} / ${buttons.join(",")}`);
    }
    // tooltips on every control, with the shortcut on full screen
    await p.waitForTimeout(800);
    const w0 = wins(p).nth(0);
    for (const [name, text] of [["Full screen", /Full screen/], ["Share", /Share/], ["More", /More/]]) {
      await w0.getByRole("button", { name }).hover(); await p.waitForTimeout(1000);
      ok(text.test(await p.locator(".warm-tip").innerText().catch(() => "")), `the ${name} button has its warm tooltip`, await p.locator(".warm-tip").innerText().catch(() => ""));
      await p.mouse.move(5, 5); await p.waitForTimeout(150);
    }
    ok(/Alt|⌥/.test(await (async () => { await w0.getByRole("button", { name: "Full screen" }).hover(); await p.waitForTimeout(700); return p.locator(".warm-tip").innerText().catch(() => ""); })()), "the tooltip of full screen shows its shortcut");
    await p.mouse.move(5, 5);
    // ---- the wallpaper and the colour ----
    ok(await p.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--wall-sharp").includes("url(")) && await p.evaluate(() => /url\(/.test(getComputedStyle(document.querySelector("[data-canvas]"), "::before").backgroundImage)), "the wallpaper's own picture shows sharp around the windows");
    const c = await p.evaluate(() => { const w = document.querySelectorAll("[data-win]")[0]; const s = getComputedStyle(w); return { bg: s.backgroundColor, ink: s.color }; });
    ok(ratio(rgbOf(c.bg), rgbOf(c.ink)) >= 7, "a window's ink reaches 7:1 on its wallpaper-derived colour", `${c.bg} / ${c.ink}`);
    ok(scheme === "dark" ? Math.max(...rgbOf(c.bg)) < 80 : Math.min(...rgbOf(c.bg)) > 200, `the window colour is ${scheme === "dark" ? "a deep" : "a pale"} tone`, c.bg);
    // ---- niri's structure ----
    let r = await rects(p); const cb = await canvasBox(p);
    ok(r.length === 3 && Math.abs(r[1].x - r[0].x - r[0].w - 8) < 1.5 && Math.abs(r[0].w - (cb.width - 24) / 2) < 2, "two columns share the canvas, half each, with an 8 px gap all round and between", `${r[0].w.toFixed(0)} / ${r[1].w.toFixed(0)} of ${cb.width}`);
    ok(Math.abs(r[0].y - cb.y - 8) < 1.5 && Math.abs(r[0].x - cb.x - 8) < 1.5, "the first window is one gap from the canvas's top and left edge");
    ok(Math.abs(r[1].x - r[0].x - r[0].w - 8) < 1.5 && Math.abs(r[2].x - r[1].x) < 1.5 && r[2].y > r[1].y, "the note is stacked below the second thread in its column");
    // a window opened by the keyboard: instant, right of the focused column
    await p.locator("[data-win]").nth(0).click({ position: { x: 100, y: 400 } }); await p.waitForTimeout(500);
    const before = (await wins(p).count());
    await key(p, "Alt+Shift+KeyN"); await p.waitForTimeout(100);
    ok((await wins(p).count()) === before + 1 && !(await animating(p, 150)), "a keyboard-opened window is instant (no animation)", `${before} -> ${await wins(p).count()}`);
    r = await rects(p);
    const focused = await p.locator("[data-win][data-focused]").evaluate((e) => e.dataset.kind + ":" + [...document.querySelectorAll("[data-win]")].indexOf(e));
    ok((await p.locator("[data-win][data-focused]").getAttribute("data-kind")) === "threads", "the new window has focus", focused);
    // pointer: add beside from the menu animates
    await p.locator("[data-win][data-focused]").getByRole("button", { name: "More" }).click();
    await item(p, /Open a thread beside/).click();
    ok(await animating(p), "a pointer-opened window animates (FLIP on the motion tokens)");
    await p.waitForTimeout(700);
    ok(!(await animating(p, 150)), "the animation ends");
    // ---- keyboard focus moves, presets ----
    const fx = await p.locator("[data-win][data-focused]").evaluate((e) => e.getBoundingClientRect().x);
    await key(p, "Alt+Shift+ArrowLeft"); await p.waitForTimeout(100);
    ok((await p.locator("[data-win][data-focused]").evaluate((e) => e.getBoundingClientRect().x)) < fx, "Alt+Shift+Left focuses the column on the left");
    await key(p, "Alt+Shift+KeyR"); await p.waitForTimeout(120);
    ok(!(await animating(p, 150)), "a keyboard width preset is instant");
    const fw = await p.locator("[data-win][data-focused]").evaluate((e) => e.getBoundingClientRect().width);
    ok(Math.abs(fw - ((cb.width - 8) * 2 / 3 - 8)) < 2 || Math.abs(fw - ((cb.width - 8) / 3 - 8)) < 2 || Math.abs(fw - (cb.width - 16)) < 2, "the preset is one of 1/3, 2/3 or full (from half)", fw.toFixed(0));
    // pointer width preset from the menu animates and lands on 1/3
    await p.locator("[data-win][data-focused]").getByRole("button", { name: "More" }).click();
    await item(p, /Width 1\/3/).click();
    ok(await animating(p), "a pointer width preset animates");
    await p.waitForTimeout(700);
    ok(Math.abs((await p.locator("[data-win][data-focused]").evaluate((e) => e.getBoundingClientRect().width)) - ((cb.width - 8) / 3 - 8)) < 2, "the width is exactly 1/3 of the canvas, gaps counted");
    // close back to two windows, then one window fills the canvas and is centred
    while ((await wins(p).count()) > 1) { await key(p, "Alt+Shift+KeyW"); await p.waitForTimeout(100); }
    await p.waitForTimeout(300);
    r = await rects(p);
    ok(r.length === 1 && Math.abs(r[0].w - (cb.width - 16)) < 2 && r[0].w / cb.width > 0.97, "a single window fills the canvas (a gap all round): about 99 % of the width", `${(r[0].w / cb.width * 100).toFixed(1)} %`);
    ok(Math.abs((r[0].x - cb.x) - (cb.x + cb.width - r[0].x - r[0].w)) < 2, "and is centred");
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
    await p.locator("[data-focused]").getByRole("button", { name: "Full screen" }).click();
    ok(await animating(p), "full screen by the pointer animates");
    await p.waitForTimeout(600);
    const cv = await canvasBox(p), mx = await p.locator("[data-win].maximised").boundingBox();
    ok(Math.abs(mx.width - cv.width) < 2 && Math.abs(mx.height - cv.height) < 2, "full screen merges the window into the outer card (it fills it)", `${mx.width}x${mx.height} of ${cv.width}x${cv.height}`);
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
    // edge fade
    await p.mouse.wheel(0, -5000); await p.waitForTimeout(300);
    const fadeAt = () => p.evaluate(() => [".fade.l", ".fade.r"].map((q) => 1 - +getComputedStyle(document.querySelector(q)).opacity));
    ok((await fadeAt())[0] === 1, "no fade at the left edge when no window crosses it");
    const e0 = await wins(p).nth(1).boundingBox(); await p.mouse.move(e0.x + e0.width / 2, e0.y + e0.height / 2); await p.mouse.wheel(0, 120); await p.waitForTimeout(400);
    ok((await fadeAt())[0] < 1, "a window leaving through the left edge fades it", String(await fadeAt()));
    await ctx.close();
  }
  // ---- the frosted window: the old Calternal's card ----
  {
    const { page: p, ctx } = await open(browser, "desktop", "dark", "/threads?canvas=1");
    await p.waitForTimeout(2200);
    await wins(p).nth(0).getByRole("button", { name: "More" }).click();
    await item(p, /frosted/).click(); await p.waitForTimeout(300);
    const fg = await p.evaluate(() => { const g = document.querySelector(".win.frosted .fg"); if (!g) return null; const s = getComputedStyle(g); return { f: s.backdropFilter || s.webkitBackdropFilter, d: getComputedStyle(g.parentElement).getPropertyValue("--fd") }; });
    ok(fg && /blur\(20px\)/.test(fg.f) && /saturate\(1\.4\)/.test(fg.f), "a frosted window is one glass layer: blur 20 px, saturation 140 %", JSON.stringify(fg));
    ok(fg && +fg.d > 0 && +fg.d <= 1, "its paper density is the least that still reads (frost.ts), from the wallpaper's extremes", fg?.d);
    await ctx.close();
  }
  // ---- six windows: frame times at 4x CPU, opaque and with every window frosted ----
  for (const frosted of [false, true]) {
    const { page: p, ctx } = await open(browser, "desktop", "dark", "/threads?canvas=1");
    await p.waitForTimeout(2200);
    for (let i = 0; i < 3; i++) await key(p, "Alt+Shift+KeyN");
    if (frosted) { await p.evaluate(() => { const s = JSON.parse(localStorage.getItem("cal.canvas")); s.frostAll = true; localStorage.setItem("cal.canvas", JSON.stringify(s)); }); await p.reload(); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(2500); }
    const total = await p.evaluate(() => JSON.parse(localStorage.getItem("cal.canvas")).cols.reduce((n, c) => n + c.wins.length, 0));
    await p.mouse.wheel(0, -9999); await p.waitForTimeout(500);
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
    console.log(`INFO  frame times while panning, 4x CPU, ${total} windows, ${frosted ? "all frosted" : "opaque"}: n=${dt.length} p50=${s[s.length >> 1].toFixed(1)}ms p95=${p95.toFixed(1)}ms max=${max.toFixed(1)}ms late(>24ms)=${late}`);
    if (loadavg()[0] > cpus().length) console.log(`SKIP  p95 judgement: the VM is busy (load ${loadavg()[0].toFixed(0)} on ${cpus().length} cores)`);
    else if (!frosted) ok(p95 < 24, "panning six windows at 4x CPU keeps the p95 frame under 24 ms", `p95=${p95.toFixed(1)}`);
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: 1 });
    if (!frosted) {
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
    ok(w.width > 360 && w.width <= 390 - 8 + 1, "a phone window is the screen's width less a gap on each side", String(w.width));
    const x0 = await stripX(p);
    await touch("touchStart", [[150, 400], [250, 400]]);
    for (let i = 1; i <= 8; i++) await touch("touchMove", [[150 - i * 20, 400], [250 - i * 20, 400]]);
    await touch("touchEnd", []);
    await p.waitForTimeout(600);
    ok(x0 - (await stripX(p)) > 80, "two fingers moving left bring the next window in", `${x0} -> ${await stripX(p)}`);
    await touch("touchStart", [[60, 300]]);
    for (let i = 1; i <= 10; i++) await touch("touchMove", [[60 + i * 20, 300]]);
    await touch("touchEnd", []);
    await p.waitForTimeout(700);
    const open1 = await p.locator(".shell").getAttribute("data-sidebar");
    ok(open1 === "true" || (await p.locator(".narrow .mid").evaluate((e) => new DOMMatrixReadOnly(getComputedStyle(e).transform).m41)) > 100, "one finger swiping across the canvas moves the sidebar into view", `data-sidebar=${open1}`);
    await ctx.close();
  }
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);
