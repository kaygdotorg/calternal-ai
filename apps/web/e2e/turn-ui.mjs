// A turn from the moment of the send: the pulsing Working row with what is happening now, the reasoning and the steps as they come, then the finished message with "Thought for" and "Worked for" folded blocks and its
// provider's mark; the thread's overflow button; the spinner's steadiness. Fixtures. Usage: CAL_URL=... node e2e/turn-ui.mjs
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
try {
  for (const device of ["desktop", "phone"]) {
    console.log(`-- ${device}`);
    const { page: p, ctx } = await open(browser, device, "dark", "/threads/t-idle");
    // the overflow button, at the card's top right, with the sidebar row's menu
    const more = p.getByRole("button", { name: "Thread actions" });
    ok((await more.count()) === 1, "the thread has an overflow button in its header");
    if (await more.count()) {
      const r = await more.boundingBox(); const vw = p.viewportSize().width;
      ok(r.x + r.width > vw * 0.5, "it sits at the right of the card header", `x=${Math.round(r.x)}`);
      await more.click(); await p.getByRole("menuitem", { name: "Rename" }).waitFor({ timeout: 5000 }).catch(() => {});
      const items = await p.getByRole("menuitem").allInnerTexts();
      ok(items.some((t) => /Rename/.test(t)) && items.some((t) => /Copy link/.test(t)) && items.some((t) => /Archive/.test(t)), "its menu is the row's menu", items.join(" | ").replace(/\s+/g, " "));
      await p.keyboard.press("Escape"); await p.waitForTimeout(300);
    }
    // a turn
    await p.locator("#composer-field").click(); await p.locator("#composer-field").fill("go"); await p.keyboard.press("Enter");
    globalThis.__shot = 0;
    const working = p.locator(".working");
    await working.waitFor({ timeout: 2000 }).catch(() => {});
    ok((await working.count()) === 1, "the Working row is there from the moment of the send", (await working.innerText().catch(() => "")).replace(/\s+/g, " "));
    const pulse = await p.locator(".working .word").evaluate((e) => ({ name: getComputedStyle(e).animationName, run: e.getAnimations().length > 0 })).catch(() => ({}));
    ok(pulse.name === "pulse" && pulse.run, "and it pulses");
    const phases = new Set(); let thought = false, steps = false;
    for (let i = 0; i < 120 && (await p.locator(".working").count()); i++) {
      phases.add((await p.locator(".working .phase").innerText().catch(() => "")).trim());
      if (await p.locator(".thought").count()) thought = true;
      if (await p.locator(".steps").count()) steps = true;
      if (thought && steps && !globalThis.__shot) { globalThis.__shot = 1; await p.screenshot({ path: `/tmp/turn-run-${device}.png` }); }
      await p.waitForTimeout(150);
    }
    console.log("   phases seen:", [...phases].filter(Boolean).join(" → "));
    ok(phases.size >= 3, "the row says what is happening as it changes");
    ok(thought && steps, "the reasoning and the steps show while it runs");
    await p.locator(".working").waitFor({ state: "detached", timeout: 60000 }).catch(() => {});
    await p.waitForTimeout(500);
    const last = p.locator(".msg.agent-row").last();
    const text = (await last.innerText()).replace(/\s+/g, " ");
    ok(/Thought for \d+ s/.test(text), "the finished message has a folded Thought for block", text.slice(0, 120));
    ok(/Worked for \d+ s · \d+ tools?/.test(text), "and a folded Worked for block with its steps inside");
    const mark = await last.locator('[role="img"]').first().getAttribute("aria-label");
    ok(/Claude Code|Codex/.test(mark ?? ""), "its provider's mark is at its left, naming provider and model", mark ?? "");
    await p.screenshot({ path: `/tmp/turn-${device}.png` });
    // the spinner: one steady speed, kept in keyboard use
    const sp = await p.evaluate(() => { const e = document.querySelector(".spin"); if (!e) return null; const a = e.getAnimations()[0]; return a ? { easing: a.effect.getTiming().easing, dur: a.effect.getTiming().duration } : { easing: "none" }; });
    if (sp) { ok(sp.easing === "linear", "a spinner turns on a linear curve", JSON.stringify(sp)); await p.keyboard.press("Tab"); await p.waitForTimeout(100); const k = await p.evaluate(() => { const e = document.querySelector(".spin"); return e ? getComputedStyle(e).animationDuration : "n/a"; }); ok(k !== "0s", "and keeps turning in keyboard use", k); }
    await ctx.close();
  }
} finally { await browser.close(); stop(); }
process.exit(bad ? 1 : 0);
