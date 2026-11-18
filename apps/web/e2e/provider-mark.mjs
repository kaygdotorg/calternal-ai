// Who wrote a message (docs/threads-ui.md, "Who wrote it"): the provider's own icon at the left of every agent message, aligned to the first line's x-height centre at every text size and zoom; hovering it (focusing it from the keyboard) opens the card
// (harness as the title, then Provider, Model, Reasoning, Service tier only when it is not the default, Context), with the warm tooltip's timing; the fixtures offer a compact icon-led design behind a switch. Usage: CAL_URL=... node e2e/provider-mark.mjs
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
/** The vertical distance (px) between the mark's centre and the first text line's x-height centre. */
const offset = (p) => p.evaluate(() => {
  const row = [...document.querySelectorAll(".msg.agent-row")].find((r) => r.querySelector(".agent .md"));
  const mark = row.querySelector("button.mark").getBoundingClientRect();
  const el = row.querySelector(".agent .md");
  const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT); let node = walker.nextNode(); while (node && !node.textContent.trim()) node = walker.nextNode();
  const range = document.createRange(); range.selectNodeContents(node);
  const line = range.getClientRects()[0]; const cs = getComputedStyle(node.parentElement);
  const c = document.createElement("canvas").getContext("2d"); c.font = `${cs.fontStyle} ${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`;
  const m = c.measureText("x"); const baseline = line.top + m.fontBoundingBoxAscent;
  const xcentre = baseline - m.actualBoundingBoxAscent / 2;
  return { d: Math.abs(mark.top + mark.height / 2 - xcentre), mark: mark.top + mark.height / 2, x: xcentre };
});
try {
  const { page: p, ctx } = await open(browser, "desktop", "dark", "/threads/t-idle");
  await p.waitForTimeout(800);
  const icons = await p.locator(".msg.agent-row button.mark svg path").evaluateAll((a) => a.map((x) => x.getAttribute("d")?.slice(0, 18)));
  ok(icons.length > 0 && icons.every((d) => d?.startsWith("M12 3v18M3 12h18") || d?.startsWith("M12 3l8 4.5")), "every agent message has its provider's own icon, never the generic spark", icons.slice(0, 3).join(" | "));
  for (const scale of [0.8, 1, 1.25, 1.5]) {
    await p.evaluate((s) => document.documentElement.style.setProperty("--ui-scale", String(s)), scale); await p.waitForTimeout(300);
    const o = await offset(p);
    ok(o.d <= 1.5, `the mark's centre is the first line's x-height centre at text size ${scale}`, `off by ${o.d.toFixed(2)} px (mark ${o.mark.toFixed(1)}, x-height centre ${o.x.toFixed(1)})`);
  }
  await p.evaluate(() => document.documentElement.style.removeProperty("--ui-scale")); await p.waitForTimeout(200);
  const mark = p.locator(".msg.agent-row button.mark").first();
  await mark.scrollIntoViewIfNeeded();
  await mark.hover(); await p.waitForTimeout(200);
  ok((await p.locator("[role=tooltip] .card").count()) === 0, "the card waits (the warm tooltip's 400 ms) before it opens");
  await p.waitForTimeout(500);
  const detailed = await p.locator("[role=tooltip] .card").first().innerText().catch(() => "");
  ok(/Claude Code|Codex/.test(detailed) && /Provider/.test(detailed) && /Model/.test(detailed) && /Reasoning/.test(detailed) && /Context/.test(detailed), "hovering opens the card: harness, Provider, Model, Reasoning, Context", detailed.replace(/\s+/g, " ").slice(0, 120));
  ok((await p.locator("[role=tooltip] [data-card=detailed]").count()) === 1, "the detailed design is the default");
  await p.locator("[role=tooltip] .swap").click(); await p.waitForTimeout(300);
  ok((await p.locator("[role=tooltip] [data-card=compact]").count()) === 1 && (await p.locator("[role=tooltip] .chips .c").count()) >= 3, "the fixtures' switch gives the compact icon-led design");
  await p.mouse.move(5, 5); await p.waitForTimeout(500);
  ok((await p.locator("[role=tooltip] .card").count()) === 0, "leaving closes it");
  // the service tier row appears only when it is not the default
  const tiers = [];
  for (const m of (await p.locator(".msg.agent-row button.mark").all()).slice(0, 6)) {
    await m.scrollIntoViewIfNeeded(); await m.hover(); await p.waitForTimeout(650);
    const t = await p.locator("[role=tooltip] .card").first().innerText().catch(() => "");
    tiers.push(/Service tier/.test(t)); await p.mouse.move(5, 5); await p.waitForTimeout(250);
  }
  ok(tiers.some(Boolean) && tiers.some((x) => !x), "Service tier shows only on a message that ran on a non-default tier", tiers.join(","));
  // keyboard
  // reach a mark by Tab, as a keyboard user does (a scripted focus() is only "keyboard focus" by the browser's heuristic)
  await p.evaluate(() => document.activeElement?.blur());
  for (let i = 0; i < 60 && !(await p.evaluate(() => document.activeElement?.classList.contains("mark"))); i++) await p.keyboard.press("Tab");
  await p.waitForTimeout(300);
  ok((await p.locator("[role=tooltip] .card").count()) === 1, "focusing the mark from the keyboard opens the card at once");
  await p.keyboard.press("Escape"); await p.waitForTimeout(300);
  ok((await p.locator("[role=tooltip] .card").count()) === 0, "Escape closes it");
  await ctx.close();
  // No hanging indent (owner, 2026-11-18): the mark is part of the first line; later lines start at the message's own left edge, on a desktop and a phone.
  for (const device of ["desktop", "phone"]) {
    const { page: q, ctx: c2 } = await open(browser, device, "dark", "/threads/t-run");
    await q.waitForTimeout(900);
    const g = await q.evaluate(() => {
      const out = {}; for (const m of document.querySelectorAll(".msg.agent-row")) m.style.maxWidth = "22rem"; // narrow, so a paragraph runs to several lines on a desktop too
      for (const kind of ["lead-text", "lead-row"]) {
        const all = [...document.querySelectorAll(".agent." + kind)]; out.classes = [...document.querySelectorAll(".msg.agent-row .agent")].map((e) => e.className).slice(0, 6); const agent = kind === "lead-text" ? all.find((e) => { const r = document.createRange(); const pe = e.querySelector(".md > p"); if (!pe) return false; r.selectNodeContents(pe); return r.getClientRects().length > 1; }) ?? all[0] : all[0]; if (!agent) { out[kind] = null; continue; }
        const mark = agent.querySelector("button.mark").getBoundingClientRect(); const a = agent.getBoundingClientRect();
        if (kind === "lead-text") {
          const pEl = agent.querySelector(".md > p"); const r = document.createRange(); r.selectNodeContents(pEl);
          const lines = []; for (const x of r.getClientRects()) { if (!lines.some((l) => Math.abs(l.top - x.top) < 4)) lines.push({ top: x.top, left: x.left }); else { const l = lines.find((l) => Math.abs(l.top - x.top) < 4); l.left = Math.min(l.left, x.left); } }
          out[kind] = { markRight: mark.right, markLeft: mark.left, agentLeft: a.left, first: lines[0].left, rest: lines.slice(1).map((l) => l.left), n: lines.length, markTop: mark.top, firstTop: lines[0].top };
        } else {
          const head = agent.querySelector(".block-head").getBoundingClientRect(); const lab = agent.querySelector(".block-head .sum").getBoundingClientRect();
          out[kind] = { markLeft: mark.left, markRight: mark.right, headLeft: head.left, headRight: head.right, labelLeft: lab.left, markMid: mark.top + mark.height / 2, headMid: head.top + head.height / 2 };
        }
      }
      return out;
    });
    const t = g["lead-text"], r = g["lead-row"];
    ok(!!t && t.markLeft >= t.agentLeft - 0.5 && t.markRight <= t.first + 0.5, `${device}: the mark is inline at the start of the first line (the text begins after it)`, JSON.stringify(t));
    ok(!!t && t.n >= 2 && t.rest.every((x) => Math.abs(x - t.agentLeft) <= 1), `${device}: no hanging indent, every later line starts at the message's left edge`, t ? t.rest.join(",") : "");
    ok(!!r && r.markLeft >= r.headLeft && r.markRight <= r.labelLeft && Math.abs(r.markMid - r.headMid) <= 1.5, `${device}: a message that starts with a row has the mark inside the row, before its label, on its centre`, JSON.stringify(r) + JSON.stringify(g.classes));
    await c2.close();
  }
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);
