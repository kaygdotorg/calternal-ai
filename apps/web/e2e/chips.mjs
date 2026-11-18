// The composer's four chips and their adaptive behaviour (docs/composer.md, decisions of 2026-11-18): attach, model (provider icon, name, reasoning level), permission, runs on (machine and a shield); as the composer's
// own width shrinks they give up words in the order Runs on, Permission, Model; a chip holding a non-default choice keeps its words longest and Full access wears a warning tint. Usage: CAL_URL=... node e2e/chips.mjs
import { launch, open, serve, expandComposer, win } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
/** For each chip (attach, model, perm, where): whether it shows words. */
const words = (p) => p.evaluate(() => [...(document.querySelector("[data-win][data-focused]") ?? document).querySelectorAll(".composer .chips .row > .chip")].map((c) => c.querySelector(".pill")?.classList.contains("wide") ?? false));
try {
  const { page: p, ctx } = await open(browser, "desktop", "dark", "/threads/t-idle");
  await expandComposer(p);
  const names = await p.evaluate(() => [...(document.querySelector("[data-win][data-focused]") ?? document).querySelectorAll(".composer .chips .row > .chip .pill")].map((e) => e.getAttribute("aria-label") ?? e.textContent.trim()));
  console.log("   chips:", names.join(" | "));
  ok(names.length === 4, "four chips: attach, model, permission, runs on");
  ok(!names.some((n) => n.includes("·")), "no middle dot in any chip's name");
  ok((await words(p)).join() === "false,true,true,true", "wide: attach is icon only, the others show words");
  const where = win(p).locator(".composer .chips .row > .chip").nth(3);
  ok((await where.locator(".lbl").innerText()).length > 3 && (await where.locator("svg").count()) >= 2, "Runs on is the machine's name and a shield (no word)", (await where.innerText()).trim());
  // a fresh thread has only default choices: shrink the composer itself (its own width, not the window's) and watch the order words go
  await p.keyboard.press("Control+k"); await p.locator(".pal-input").fill("new thread"); await p.waitForTimeout(250);
  await p.locator("[role=option]", { hasText: "New thread" }).first().click(); await p.waitForURL(/\/threads\/t-new/); await p.waitForTimeout(600);
  await expandComposer(p);
  console.log("   fresh thread chips:", (await p.evaluate(() => [...document.querySelectorAll(".composer .chips .row > .chip .pill")].map((e) => e.getAttribute("aria-label") ?? e.textContent.trim()))).join(" | "));
  const seq = [];
  for (const w of [900, 700, 640, 560, 520, 480, 440, 420, 380, 300]) {
    await p.addStyleTag({ content: `.bottom .inner { max-width: ${w}px !important; }` }); await p.waitForTimeout(300);
    seq.push((await words(p)).slice(1)); 
  }
  console.log("   [model perm where] as the composer narrows:", seq.map((s) => s.map((x) => (x ? "T" : "i")).join("")).join(" "));
  const monotone = seq.every((s, i) => i === 0 || s.every((x, k) => !x || seq[i - 1][k]));
  ok(monotone, "a chip that gave up its words does not take them back as the composer narrows");
  const at = (k) => seq.findIndex((s) => !s[k]);
  ok(at(2) >= 0 && at(2) <= at(1) && at(1) <= at(0) && at(0) >= 0, "Runs on gives up its words first, then Permission, then Model", `at steps ${at(2)}, ${at(1)}, ${at(0)}`);
  ok(seq.at(-1).join() === "false,false,false", "narrowest: all icon only", seq.at(-1).join());
  await p.evaluate(() => document.querySelectorAll("style").forEach((e) => { if (e.textContent.includes(".bottom .inner { max-width")) e.remove(); }));
  // a non-default choice keeps its words; Full access is tinted
  await p.addStyleTag({ content: ".bottom .inner { max-width: 700px !important; }" }); await p.waitForTimeout(300);
  await expandComposer(p);
  await win(p).locator(".composer .chips .row > .chip .pill").nth(2).click(); await p.getByRole("menuitemradio", { name: /Full access/ }).click(); await p.waitForTimeout(400);
  await p.addStyleTag({ content: ".bottom .inner { max-width: 330px !important; }" }); await p.waitForTimeout(350); await expandComposer(p);
  const w2 = (await words(p)).slice(1);
  ok(w2[1] === true && w2[0] === false && w2[2] === false, "at the narrowest only the chip holding a non-default choice keeps its words", w2.join());
  const tint = await p.evaluate(() => { const c = document.querySelector(".composer .chips .row > .chip.warn .pill"); return c ? getComputedStyle(c).color : null; });
  ok(!!tint, "Full access wears a warning tint", tint ?? "");
  await p.screenshot({ path: "/tmp/chips-narrow.png" });
  await ctx.close();
} finally { await browser.close(); stop(); }
process.exit(bad ? 1 : 0);
