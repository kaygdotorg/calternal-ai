// The canvas experiment (docs/ui-shell-status.md, "The canvas experiment"), behind ?canvas=1 in the fixtures build. Usage: CAL_URL=... node e2e/canvas.mjs
// Checks: off without the flag; windows (threads twice, a note); the wallpaper shown sharp around them; the window colour from the wallpaper with ink at 7:1; focus follows the press and the address and sidebars follow focus;
// the wheel rule (focused: inside; unfocused: the canvas); the input lock; resize by edges; maximise and restore; deep links (one window per address); only near columns mounted; frame times while panning six windows at 4x CPU.
import { launch, open, serve, BASE } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const wins = (p) => p.locator("[data-win]");
const stripX = (p) => p.evaluate(() => new DOMMatrixReadOnly(getComputedStyle(document.querySelector(".strip")).transform).m41);
const lum = ([r, g, b]) => { const f = (v) => ((v /= 255) <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4); return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b); };
const rgbOf = (s) => (s.match(/[\d.]+/g) ?? []).slice(0, 3).map(Number);
const ratio = (a, b) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); };
try {
  // off without the flag
  { const { page, ctx } = await open(browser, "desktop", "dark", "/threads/t-idle"); ok((await page.locator("[data-canvas]").count()) === 0, "without ?canvas=1 nothing changes (no canvas)"); await ctx.close(); }
  for (const scheme of ["dark", "light"]) {
    console.log(`-- ${scheme}`);
    const { page, ctx } = await open(browser, "desktop", scheme, "/threads/t-idle?canvas=1");
    await page.waitForTimeout(1500);
    ok((await wins(page).count()) === 3, "three windows: a thread, another thread and a note", String(await wins(page).count()));
    const kinds = await wins(page).evaluateAll((w) => w.map((x) => x.dataset.kind));
    ok(kinds.filter((k) => k === "threads").length === 2 && kinds.includes("notes"), "the same app is open twice and another app beside it", kinds.join(","));
    ok(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--wall-sharp").includes("url(")), "the wallpaper's own picture is there for the sharp area");
    ok(await page.evaluate(() => /url\(/.test(getComputedStyle(document.querySelector("[data-canvas]"), "::before").backgroundImage)), "the area around the windows shows the sharp picture");
    // colours
    const c = await page.evaluate(() => { const w = document.querySelectorAll("[data-win]")[0]; const s = getComputedStyle(w); return { bg: s.backgroundColor, ink: s.color, ink3: getComputedStyle(w).getPropertyValue("--ink-3") }; });
    const bg = rgbOf(c.bg), ink = rgbOf(c.ink);
    ok(ratio(bg, ink) >= 7, "a window's ink reaches 7:1 on its wallpaper-derived colour", `${c.bg} / ${c.ink} = ${ratio(bg, ink).toFixed(1)}`);
    ok(scheme === "dark" ? Math.max(...bg) < 80 : Math.min(...bg) > 200, `the window colour is ${scheme === "dark" ? "a deep" : "a pale"} tone`, c.bg);
    // focus follows the press; address and sidebar follow
    const url0 = new URL(page.url()).pathname;
    const side0 = await page.locator("aside.side").innerText();
    await wins(page).nth(2).click({ position: { x: 200, y: 200 } });
    await page.waitForTimeout(600);
    ok((await wins(page).nth(2).getAttribute("data-focused")) !== null && /^\/notes\//.test(new URL(page.url()).pathname), "pressing a window focuses it and the address follows it", new URL(page.url()).pathname);
    ok((await page.locator("aside.side").innerText()) !== side0, "the left sidebar follows the focused window's app");
    await wins(page).nth(0).click({ position: { x: 200, y: 300 } });
    await page.waitForTimeout(600);
    ok(new URL(page.url()).pathname === url0, "pressing the first window returns the address", new URL(page.url()).pathname);
    // wheel rule
    const x0 = await stripX(page);
    const b = await wins(page).nth(1).boundingBox();
    await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
    await page.mouse.wheel(0, 300); await page.waitForTimeout(300);
    const x1 = await stripX(page);
    ok(Math.abs(x1 - x0) > 50, "a wheel over an unfocused window moves the canvas", `${x0} -> ${x1}`);
    await page.mouse.wheel(0, -2000); await page.waitForTimeout(200);
    const f = await page.locator("[data-win][data-focused]").boundingBox();
    const x2 = await stripX(page);
    await page.mouse.move(f.x + f.width / 2, f.y + f.height / 2);
    await page.mouse.wheel(0, 200); await page.waitForTimeout(300);
    ok(Math.abs((await stripX(page)) - x2) < 1, "a wheel over the focused window never moves the canvas");
    // the edge fade: none at rest at the start, present while a window is cut off at an edge
    const fadeAt = () => page.evaluate(() => [".fade.l", ".fade.r"].map((q) => 1 - +getComputedStyle(document.querySelector(q)).opacity));
    await page.mouse.wheel(0, -3000); await page.waitForTimeout(400);
    const f0 = await fadeAt();
    ok(f0[0] === 1, "no fade at the left edge when no window crosses it", String(f0));
    await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
    await page.mouse.wheel(0, 120); await page.waitForTimeout(400);
    const f1 = await fadeAt();
    ok(f1[0] < 1, "a window leaving through the left edge fades it", String(f1));
    await page.mouse.wheel(0, -3000); await page.waitForTimeout(400);
    // the lock
    const w1 = wins(page).nth(1);
    await w1.click({ position: { x: 200, y: 200 } }); await page.waitForTimeout(500);
    const bb = await w1.boundingBox();
    await page.mouse.move(bb.x + bb.width / 2, bb.y + 8);
    await page.waitForTimeout(300);
    ok((await w1.locator(".lock").evaluate((e) => +getComputedStyle(e).opacity)) > 0.3, "the padlock appears at the window's top middle when the pointer is near");
    await w1.locator(".lock").click();
    ok((await w1.getAttribute("data-locked")) !== null && (await w1.locator(".lock").getAttribute("aria-pressed")) === "true", "the input lock locks the window");
    const xl = await stripX(page);
    const o = await wins(page).nth(0).boundingBox();
    await page.mouse.move(o.x + o.width / 2, o.y + o.height / 2);
    await page.mouse.wheel(0, 400); await page.waitForTimeout(300);
    ok(Math.abs((await stripX(page)) - xl) < 1, "while locked, the canvas does not move");
    await w1.locator(".lock").click();
    ok((await w1.getAttribute("data-locked")) === null, "unlocking brings the canvas back");
    // resize
    const w0 = wins(page).nth(0);
    const r0 = await w0.boundingBox();
    await page.mouse.move(r0.x + r0.width + 1, r0.y + 200);
    await page.mouse.down(); await page.mouse.move(r0.x + r0.width + 91, r0.y + 200, { steps: 6 }); await page.mouse.up();
    const r1 = await w0.boundingBox();
    ok(Math.abs(r1.width - (r0.width + 90)) < 4, "dragging the right edge resizes the window", `${r0.width} -> ${r1.width}`);
    const t0 = await wins(page).nth(1).boundingBox(), t1 = await wins(page).nth(2).boundingBox();
    await page.mouse.move(t0.x + t0.width / 2, t0.y + t0.height + 1);
    await page.mouse.down(); await page.mouse.move(t0.x + t0.width / 2, t0.y + t0.height - 59, { steps: 6 }); await page.mouse.up();
    const u0 = await wins(page).nth(1).boundingBox(), u1 = await wins(page).nth(2).boundingBox();
    ok(u0.height < t0.height - 40 && u1.height > t1.height + 40, "dragging the boundary between stacked windows resizes both", `${t0.height.toFixed(0)}/${t1.height.toFixed(0)} -> ${u0.height.toFixed(0)}/${u1.height.toFixed(0)}`);
    // maximise and restore
    await wins(page).nth(0).click({ position: { x: 380, y: 200 } });
    await page.locator("[data-focused] button[aria-label='Maximise the window']").click(); await page.waitForTimeout(500);
    const cv = await page.locator("[data-canvas]").boundingBox(), mx = await page.locator("[data-win].maximised").boundingBox();
    ok(Math.abs(mx.width - cv.width) < 2 && Math.abs(mx.height - cv.height) < 2, "maximise merges the window into the outer card (fills it)", `${mx.width}x${mx.height} of ${cv.width}x${cv.height}`);
    await page.locator("[data-win].maximised button[aria-label='Restore the window']").click(); await page.waitForTimeout(500);
    ok((await page.locator("[data-win].maximised").count()) === 0 && (await wins(page).count()) === 3, "restore puts it back");
    // deep links: one window per address
    const n = await wins(page).count();
    const go = (id) => page.evaluate((i) => { history.pushState(null, "", location.pathname.replace(/[^/]*$/, i)); dispatchEvent(new PopStateEvent("popstate")); }, id);
    await go("t-run"); await page.waitForTimeout(500);
    const n1 = await wins(page).count();
    await go("t-idle"); await page.waitForTimeout(300); await go("t-run"); await page.waitForTimeout(500);
    ok(n1 <= n + 1 && (await wins(page).count()) === n1, "opening an address twice makes one window and focuses it", `${n} -> ${n1} -> ${await wins(page).count()}`);
    ok(await page.evaluate(() => document.querySelector("[data-win][data-focused]")?.getAttribute("aria-label")?.length > 0) && /t-run$/.test(new URL(page.url()).pathname), "the address's window has focus");
    await ctx.close();
  }
  // a phone: one window per screen; two fingers move the canvas in any direction, one finger scrolls inside the window
  {
    const { page, ctx } = await open(browser, "phone", "dark", "/threads/t-idle?canvas=1");
    await page.waitForTimeout(1500);
    const cdp = await ctx.newCDPSession(page);
    const touch = (type, pts) => cdp.send("Input.dispatchTouchEvent", { type, touchPoints: pts.map(([x, y], id) => ({ x, y, id })) });
    const x0 = await stripX(page);
    await touch("touchStart", [[150, 400], [250, 400]]);
    for (let i = 1; i <= 8; i++) await touch("touchMove", [[150 - i * 20, 400], [250 - i * 20, 400]]);
    await touch("touchEnd", []);
    await page.waitForTimeout(600);
    const x1 = await stripX(page);
    ok(x0 - x1 > 80, "on a phone, two fingers moving left bring the next window in", `${x0} -> ${x1}`);
    const w = await wins(page).first().boundingBox();
    ok(w.width > 330 && w.width <= 390, "a phone window is almost the screen's width", String(w.width));
    await ctx.close();
  }
  // six windows: only near columns are mounted; frame times while panning at 4x CPU
  {
    const { page, ctx } = await open(browser, "desktop", "dark", "/threads/t-idle?canvas=1");
    await page.waitForTimeout(1500);
    for (let i = 0; i < 3; i++) { await page.getByRole("button", { name: "Open another window" }).click(); await page.getByRole("menuitem").first().click(); await page.waitForTimeout(400); }
    const total = await page.evaluate(() => document.querySelectorAll("[data-col]").length);
    await page.evaluate(() => document.querySelector("[data-canvas]").dispatchEvent(new WheelEvent("wheel", { deltaY: -99999, bubbles: true, cancelable: true })));
    await page.waitForTimeout(500);
    const mounted = await page.evaluate(() => document.querySelectorAll("[data-win]").length);
    const wcount = await page.evaluate(() => document.querySelectorAll("[data-win]").length);
    ok(mounted < 6 + 1 && mounted >= 2, "windows far from the screen are not mounted (not laid out, not painted)", `${mounted} windows in the DOM; columns after opening: ${total}`);
    if (process.env.CANVAS_NOFADE) await page.addStyleTag({ content: ".fade { display: none !important; }" }); // a control for the fade strips
    const cdp = await ctx.newCDPSession(page);
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
    await page.mouse.move(700, 400);
    await page.evaluate(() => { window.__f = []; let l = performance.now(); const f = (t) => { window.__f.push(t - l); l = t; window.__r = requestAnimationFrame(f); }; window.__r = requestAnimationFrame(f); });
    const wins6 = await page.evaluate(() => document.querySelectorAll("[data-win]").length);
    for (let i = 0; i < 120; i++) { await page.mouse.wheel(i < 60 ? 60 : -60, 0); await page.waitForTimeout(16); }
    await page.waitForTimeout(300);
    const dt = await page.evaluate(() => { cancelAnimationFrame(window.__r); return window.__f.slice(5); });
    const s = [...dt].sort((a, b) => a - b); const p95 = s[Math.floor(s.length * 0.95)], max = s.at(-1), late = dt.filter((v) => v > 24).length;
    console.log(`INFO  frame times while panning, 4x CPU, ${total} columns, ${wins6} mounted: n=${dt.length} p50=${s[s.length >> 1].toFixed(1)}ms p95=${p95.toFixed(1)}ms max=${max.toFixed(1)}ms late(>24ms)=${late}`);
    ok(p95 < 24, "panning at 4x CPU keeps the p95 frame under 24 ms", `p95=${p95.toFixed(1)}`);
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: 1 });
    await ctx.close();
  }
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);
