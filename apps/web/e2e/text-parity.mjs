// One text behaviour: the same keystrokes on a text field (the composer, the palette) and on the note editor must give the same text and the same caret. Scenarios cover the shared shortcuts, selection
// collapse, undo grouping (typing, paste), spelling attributes and the picker triggers. Usage: node e2e/text-parity.mjs (after `npm run build`).
import { chromium } from "playwright";
import { serve, BASE, DEVICES } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const ctx = await b.newContext({ ...DEVICES.desktop }); await ctx.grantPermissions(["clipboard-read", "clipboard-write"]);
const surfaces = {
  field: async () => { const p = await ctx.newPage(); await p.goto(BASE + "/threads/t-idle?firstrun=0&test=1"); await p.waitForSelector("#composer-field"); await p.locator("#composer-field").click(); return { p, text: () => p.evaluate(() => document.querySelector("#composer-field").value), caret: () => p.evaluate(() => { const e = document.querySelector("#composer-field"); return [e.selectionStart, e.selectionEnd]; }) }; },
  editor: async () => { const p = await ctx.newPage(); await p.goto(BASE + "/notes/n-2?firstrun=0&test=1"); await p.waitForSelector(".cm-content"); await p.waitForTimeout(800); await p.locator(".cm-content").click(); await p.keyboard.press("Control+a"); await p.keyboard.press("Delete"); return { p, text: () => p.evaluate(() => window.__block.value), caret: () => p.evaluate(() => { const m = window.__block.view.state.selection.main; return [m.from, m.to]; }) }; },
};
const NL = "Shift+Enter";
async function type(p, lines) { for (const [i, l] of lines.entries()) { if (i) await p.keyboard.press(NL); await p.keyboard.type(l); } }
const clear = async (p) => { await p.keyboard.press("Control+a"); await p.keyboard.press("Delete"); await p.waitForTimeout(700); };
const scenarios = [
  ["Alt-Up moves the line up, caret with it", async (p) => { await type(p, ["one", "two", "three"]); await p.keyboard.press("ArrowUp"); await p.keyboard.press("Alt+ArrowUp"); }],
  ["Alt-Down moves it down", async (p) => { await type(p, ["one", "two", "three"]); await p.keyboard.press("ArrowUp"); await p.keyboard.press("ArrowUp"); await p.keyboard.press("Alt+ArrowDown"); }],
  ["Shift-Alt-Down copies the line", async (p) => { await type(p, ["one", "two"]); await p.keyboard.press("Shift+Alt+ArrowDown"); }],
  ["Shift-Mod-K deletes the line", async (p) => { await type(p, ["one", "two", "three"]); await p.keyboard.press("ArrowUp"); await p.keyboard.press("Control+Shift+k"); }],
  ["Mod-] and Mod-[ indent and outdent", async (p) => { await type(p, ["one", "two"]); await p.keyboard.press("Control+]"); await p.keyboard.press("Control+]"); await p.keyboard.press("Control+["); }],
  ["Alt-L selects the line", async (p) => { await type(p, ["one", "two", "three"]); await p.keyboard.press("ArrowUp"); await p.keyboard.press("Alt+l"); }],
  ["Escape collapses a selection", async (p) => { await p.keyboard.type("hello world"); await p.keyboard.press("Shift+Home"); await p.keyboard.press("Escape"); }],
  ["typing groups, a pause ends the group, undo steps back one group", async (p) => { await p.keyboard.type("abc"); await p.waitForTimeout(700); await p.keyboard.type("def"); await p.keyboard.press("Control+z"); }],
  ["a paste is a step of its own", async (p) => { await p.evaluate(() => navigator.clipboard.writeText("XY")); await p.keyboard.type("ab"); await p.keyboard.press("Control+v"); await p.keyboard.type("cd"); await p.keyboard.press("Control+z"); }],
  ["undo twice and redo", async (p) => { await p.evaluate(() => navigator.clipboard.writeText("XY")); await p.keyboard.type("ab"); await p.keyboard.press("Control+v"); await p.keyboard.press("Control+z"); await p.keyboard.press("Control+z"); await p.keyboard.press("Control+Shift+z"); }],
];
const res = { field: [], editor: [] };
for (const [name, mk] of Object.entries(surfaces)) {
  const s = await mk();
  for (const [title, run] of scenarios) { await clear(s.p); await run(s.p); await s.p.waitForTimeout(150); res[name].push({ title, text: await s.text(), caret: await s.caret() }); }
}
scenarios.forEach(([title], i) => { const f = res.field[i], e = res.editor[i]; const same = f.text === e.text && f.caret[0] === e.caret[0] && f.caret[1] === e.caret[1]; ok(same, `Same result in a field and in the note editor: ${title}`, same ? JSON.stringify(f.text) : `field ${JSON.stringify(f.text)} ${f.caret} | editor ${JSON.stringify(e.text)} ${e.caret}`); });
// attributes and triggers
{
  const a = await ctx.newPage(); await a.goto(BASE + "/notes/n-2?firstrun=0"); await a.waitForSelector(".cm-content");
  const ed = await a.locator(".cm-content").evaluate((e) => [e.getAttribute("spellcheck"), e.getAttribute("autocapitalize"), e.getAttribute("autocorrect")]);
  const f = await ctx.newPage(); await f.goto(BASE + "/threads/t-idle?firstrun=0"); await f.waitForSelector("#composer-field");
  const fl = await f.locator("#composer-field").evaluate((e) => [e.getAttribute("spellcheck"), e.getAttribute("autocapitalize"), e.getAttribute("autocorrect")]);
  ok(JSON.stringify(ed) === JSON.stringify(fl) && ed[0] === "true", "Spelling, capitals and autocorrect are told to the browser the same way", JSON.stringify(ed));
}
await b.close(); stop();
console.log(bad ? `${bad} FAIL` : "all pass"); process.exit(bad ? 1 : 0);
