// The owner's phone feedback of 2026-11-18, as checks: a tap anywhere in the collapsed composer focuses and expands it; the one round control in every state (idle, ready, stop) is the same size and on the field's line at every text size; the secret request is one line, one field,
// one action and a quiet "Not now" with the rest behind a disclosure; the Inbox and All Threads tabs split the sidebar's width in halves; a canvas window has no shell button over its pills and its menu opens the side panel. Usage: CAL_URL=... node e2e/phone-ui.mjs
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const centre = (p, sel) => p.locator(sel).first().evaluate((e) => { const r = e.getBoundingClientRect(); return { cy: r.top + r.height / 2, w: r.width, h: r.height, x: r.left, r: r.right }; });
/** The vertical centre of the field's first line box (what the placeholder or the first line of text sits in). */
const lineCentre = (p) => p.evaluate(() => { const f = document.querySelector("#composer-field"); const cs = getComputedStyle(f); const r = f.getBoundingClientRect(); const pad = parseFloat(cs.paddingTop); const lh = parseFloat(cs.lineHeight) || parseFloat(cs.fontSize) * 1.45; return r.top + pad + lh / 2; });
try {
  {
    const { page: p, ctx } = await open(browser, "phone", "dark", "/threads/t-idle");
    await p.waitForTimeout(800);
    ok((await p.locator(".composer[data-collapsed]").count()) === 1, "the composer is collapsed at rest");
    const box = await p.locator(".composer").boundingBox();
    await p.touchscreen.tap(box.x + 6, box.y + box.height / 2); await p.waitForTimeout(500);
    ok((await p.locator(".composer[data-collapsed]").count()) === 0 && (await p.evaluate(() => document.activeElement?.id)) === "composer-field", "a tap in the box's own padding focuses the field and expands it");
    await p.locator("#composer-field").blur(); await p.waitForTimeout(500);
    const field = await p.locator("#composer-field").boundingBox();
    await p.touchscreen.tap(field.x + 20, field.y + field.height / 2); await p.waitForTimeout(500);
    ok((await p.locator(".composer[data-collapsed]").count()) === 0, "a tap on the field expands it");
    await p.locator("#composer-field").blur(); await p.waitForTimeout(500);
    // idle: the quiet circle; the field's first line and the circle share one centre, at every text size
    for (const scale of [0.9, 1, 1.25, 1.5]) {
      await p.evaluate((s) => document.documentElement.style.setProperty("--ui-scale", String(s)), scale); await p.waitForTimeout(250);
      const go = await centre(p, ".go"); const lc = await lineCentre(p);
      ok(go.w === go.h && Math.abs(go.cy - lc) <= 1.5, `idle: the control is round and its centre is the first line's centre at text size ${scale}`, `button ${go.cy.toFixed(1)} line ${lc.toFixed(1)} ${go.w}x${go.h}`);
    }
    await p.evaluate(() => document.documentElement.style.removeProperty("--ui-scale"));
    ok((await p.locator('.go[data-state="idle"]').count()) === 1 && (await p.locator(".go").isDisabled()), "idle: nothing to send, the control is quiet and disabled");
    await p.locator("#composer-field").click(); await p.keyboard.type("hi"); await p.waitForTimeout(300);
    ok((await p.locator('.go[data-state="ready"]').count()) === 1 && !(await p.locator(".go").isDisabled()), "ready: with text, the control is the accent circle with an arrow");
    await ctx.close();
  }
  {
    const { page: p, ctx } = await open(browser, "phone", "dark", "/threads/t-run");
    await p.waitForTimeout(800);
    const st = await p.locator(".go").first().getAttribute("data-state");
    const g = await p.locator(".go svg rect").count();
    const cs = await p.locator(".go").first().evaluate((e) => { const s = getComputedStyle(e); return { bg: s.backgroundColor, fg: s.color }; });
    ok(st === "stop" && g === 1, "running: the control is Stop, a solid rounded square on the ink circle (the weight of ChatGPT's and bb's)", `${st} ${JSON.stringify(cs)}`);
    const go = await centre(p, ".go"); const lc = await lineCentre(p);
    ok(Math.abs(go.cy - lc) <= 1.5, "Stop sits on the field's line", `${go.cy.toFixed(1)} / ${lc.toFixed(1)}`);
    await ctx.close();
  }
  {
    const { page: p, ctx } = await open(browser, "phone", "dark", "/threads/t-secret");
    await p.waitForTimeout(1000);
    const bar = p.getByRole("region", { name: "Waiting on you" });
    const txt = (await bar.innerText()).replace(/\s+/g, " ");
    ok(/The agent asks for your Forgejo access token/.test(txt), "the secret request says in one line what is asked and by whom", txt.slice(0, 90));
    ok((await bar.locator("input[type=password]").count()) === 1 && (await bar.getByRole("button", { name: "Save" }).count()) === 1 && (await bar.getByRole("button", { name: "Not now" }).count()) === 1, "one field, one action and a quiet Not now");
    ok((await bar.locator("details.more[open]").count()) === 0 && (await bar.locator("details.more summary").count()) === 1, "the explanation is behind a disclosure, closed");
    ok(!/Drawn by Calternal|Personal|Required|FORGEJO_TOKEN|handle/.test(txt), "none of the explanation shows until it is opened", txt.slice(0, 160));
    await ctx.close();
  }
  {
    const { page: p, ctx } = await open(browser, "desktop", "dark", "/threads/t-idle");
    await p.waitForTimeout(800);
    const nav = await centre(p, "aside.side [role=tablist]");
    const a = await centre(p, "aside.side [role=tab]:nth-child(1)"), b = await centre(p, "aside.side [role=tab]:nth-child(2)");
    ok(Math.abs(a.w - b.w) <= 2 && Math.abs((a.w + b.w) - (nav.w - 16)) <= 12, "the Inbox and All Threads tabs are equal halves of the sidebar's width", `${a.w.toFixed(0)} + ${b.w.toFixed(0)} of ${nav.w.toFixed(0)}`);
    await ctx.close();
  }
  {
    const { page: p, ctx } = await open(browser, "phone", "dark", "/threads?canvas=1");
    await p.waitForTimeout(2200);
    const right = await p.evaluate(() => { const pill = document.querySelector("[data-win] .tr"); const r = pill.getBoundingClientRect(); return { l: r.left, r: r.right, t: r.top, b: r.bottom, over: [...document.querySelectorAll(".edge.r, .edge.l")].filter((e) => { const q = e.getBoundingClientRect(); return !(q.right < r.left || q.left > r.right || q.bottom < r.top || q.top > r.bottom); }).length }; });
    ok(right.over === 0, "nothing from the shell sits over a window's top-right pill on a phone", JSON.stringify(right));
    await p.locator("[data-win]").first().getByRole("button", { name: "More" }).click();
    ok((await p.locator("[role^=menuitem]", { hasText: /side panel/ }).count()) === 1, "the window's menu has the side panel item");
    await p.locator("[role^=menuitem]", { hasText: /side panel/ }).click(); await p.waitForTimeout(700);
    ok((await p.locator("aside.right [role=tab], aside.right .tabs button, aside.right").count()) > 0 && (await p.locator(".narrow").getAttribute("data-right")) === "true", "choosing it opens the side panel");
    await ctx.close();
  }
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);
