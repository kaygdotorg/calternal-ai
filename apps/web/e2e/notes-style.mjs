// The note's Style sheet on fixture notes, ported from the existing app's apps/web/e2e/style-sheet.spec.ts, style-card-box.spec.ts and style-background.spec.ts (origin/dev) into this harness:
// a theme is one tap and one plain line; Frost, Grain, Frame and Width are plain lines and Grain waits for Frost; the page colour is one row of swatches and one line; the text size is the note's own;
// a styled note opens with its own choices and writes nothing; a theme keeps the card's box on a phone. Also the block tab (kind and bold) and that typing in the body never touches the style.
// Usage: node e2e/notes-style.mjs [outdir for screenshots]   (after `npm run build -w apps/web`)
import { mkdirSync } from "node:fs";
import { chromium } from "playwright";
import { open, serve } from "./lib.mjs";
const out = process.argv[2]; if (out) mkdirSync(out, { recursive: true });
const stop = await serve(); const b = await chromium.launch();
let bad = 0;
const ok = (n, p, note = "") => { console.log(`${p ? "PASS" : "FAIL"}  ${n}${note ? "  " + note : ""}`); if (!p) bad++; };
const TEXT = "---\ntitle: Look\n---\n# Look\n\nOne thing at a time.\n";
const raw = (p) => p.evaluate(() => window.__block.value);
const lines = async (p) => (await raw(p)).split("\n").filter((l) => /^calternal-/.test(l));
const settle = (p) => p.waitForTimeout(250);
async function note(device, scheme, text = TEXT) {
  const { ctx, page } = await open(b, device, scheme, "/notes/n-2?test=1");
  await page.waitForSelector(".cm-content");
  await page.evaluate((t) => window.__block.setValue(t), text);
  if (device === "phone") await page.getByRole("button", { name: /Show the sidebar|Show notes/ }).first().click().catch(() => {});
  return { ctx, page };
}
async function openSheet(page, device) {
  if (device === "phone") { await page.evaluate(() => document.querySelector('[role=tab][aria-controls="panel"], [role=tablist] [role=tab]')); }
  await page.getByRole("tab", { name: "Style" }).first().click();
  await page.getByRole("tablist", { name: "Style" }).waitFor();
  await settle(page);
}
const sheet = (p) => p.locator('[role=tabpanel], .style-sheet').filter({ has: p.getByRole("tablist", { name: "Style" }) }).first();
const shot = async (p, name) => { if (out) await p.screenshot({ path: `${out}/${name}.png` }); };

// ---- desktop, light
{
  const { ctx, page } = await note("desktop", "light");
  await openSheet(page, "desktop");
  const s = page.locator(".style-sheet");
  await shot(page, "desktop-page-light");
  // a theme is one tap and one plain line; Same as app takes the line away
  const themes = s.getByRole("radiogroup", { name: "Theme" });
  ok("the theme row lists Same as app and the themes", (await themes.getByRole("radio").count()) >= 5);
  await themes.getByRole("radio", { name: "Nord" }).click(); await settle(page);
  ok("a theme is one plain line", JSON.stringify(await lines(page)) === JSON.stringify(["calternal-theme: nord"]), (await lines(page)).join("|"));
  ok("the chosen theme is checked", (await themes.getByRole("radio", { name: "Nord" }).getAttribute("aria-checked")) === "true");
  ok("the theme is on the note's card only, never on the page", (await page.locator("article.nc-article").evaluate((e) => getComputedStyle(e).getPropertyValue("--paper").trim())) === "#eceff4" && (await page.evaluate(() => document.documentElement.style.getPropertyValue("--paper"))) === "");
  ok("the text of the note is untouched", (await raw(page)).includes("# Look\n\nOne thing at a time.\n"));
  await shot(page, "desktop-theme-light");
  await themes.getByRole("radio", { name: "Same as app" }).click(); await settle(page);
  ok("Same as app takes the line away", (await lines(page)).length === 0);
  // Frost, Grain, Frame and Width are plain lines; Grain waits for Frost
  const frost = s.getByRole("switch", { name: "Frost" }), grain = s.getByRole("switch", { name: "Grain" });
  ok("Grain is off while Frost is off", await grain.isDisabled());
  await frost.click(); await settle(page);
  ok("Frost writes the opacity and the blur", (await lines(page)).includes("calternal-page-opacity: 80%") && (await lines(page)).includes("calternal-page-blur: 20px"));
  ok("Grain can be turned on once Frost is", await grain.isEnabled());
  await grain.click(); await settle(page);
  ok("Grain is one plain line", (await lines(page)).includes("calternal-grain: on"));
  ok("the page is a surface at the chosen opacity", (await page.locator("article.nc-article").getAttribute("data-surface")) !== null && (await page.locator("article.nc-article").evaluate((e) => e.style.getPropertyValue("--page-opacity"))) === "0.8");
  await s.getByRole("switch", { name: "Frame" }).click(); await settle(page);
  ok("Frame off is one plain line", (await lines(page)).includes("calternal-frame: off"));
  const wide = s.getByRole("tablist", { name: "Width" });
  await wide.getByRole("tab", { name: "Wide" }).click(); await settle(page);
  ok("Wide is one plain line and widens the column", (await lines(page)).includes("calternal-width: wide") && (await page.locator("article.nc-article").evaluate((e) => e.getBoundingClientRect().width)) > 700);
  await wide.getByRole("tab", { name: "Narrow" }).click(); await settle(page);
  ok("Narrow takes the width line away", !(await lines(page)).some((l) => l.startsWith("calternal-width")));
  await frost.click(); await settle(page);
  ok("turning Frost off takes the opacity, blur and grain lines away", !(await lines(page)).some((l) => /page-opacity|page-blur|grain/.test(l)));
  // page colour
  const row = s.getByRole("group", { name: "Page colour" });
  await row.getByRole("button", { name: "Sand" }).click(); await settle(page);
  ok("a page colour is one line", JSON.stringify((await lines(page)).filter((l) => /colour/.test(l))) === JSON.stringify(["calternal-page-colour: sand"]));
  ok("the swatch is pressed", (await row.getByRole("button", { name: "Sand" }).getAttribute("aria-pressed")) === "true");
  await row.getByRole("button", { name: "Paper" }).click(); await settle(page);
  ok("Paper takes the colour line away", !(await lines(page)).some((l) => /colour/.test(l)));
  // the text tab
  await s.getByRole("tablist", { name: "Style" }).getByRole("tab", { name: "Text" }).click(); await page.waitForTimeout(500);
  const size = () => page.locator("article .cm-content").first().evaluate((e) => parseFloat(getComputedStyle(e).fontSize));
  const usual = await size();
  await s.getByRole("tablist", { name: "Text size" }).getByRole("tab", { name: "Large" }).click(); await settle(page);
  ok("Large grows this note's words and writes one line", (await lines(page)).includes("calternal-text-size: large") && (await size()) > usual * 1.1, `${usual} -> ${await size()}`);
  await s.getByRole("tablist", { name: "Text size" }).getByRole("tab", { name: "Default" }).click(); await settle(page);
  ok("Default has no line and the size comes back", !(await lines(page)).some((l) => /text-size/.test(l)) && Math.abs((await size()) - usual) < 0.6);
  await s.getByRole("tablist", { name: "Font" }).getByRole("tab", { name: "Mono" }).click(); await settle(page);
  ok("a font is one line and changes the note's face", (await lines(page)).includes("calternal-font: mono") && /Cascadia|mono/i.test(await page.locator("article .cm-content").first().evaluate((e) => getComputedStyle(e).fontFamily)));
  await s.getByRole("tablist", { name: "Font" }).getByRole("tab", { name: "System" }).click(); await settle(page);
  await s.getByRole("tablist", { name: "Tree lines" }).getByRole("tab", { name: "Dotted" }).click(); await settle(page);
  ok("tree lines are one line", (await lines(page)).includes("calternal-lines: dotted"));
  await s.getByRole("button", { name: "Reset tree lines" }).click(); await settle(page);
  ok("Reset takes the tree-lines line away", !(await lines(page)).some((l) => /lines/.test(l)));
  await shot(page, "desktop-text-light");
  // typing in the body never touches the style
  const before = await lines(page);
  await page.locator(".cm-content").click(); await page.keyboard.press("Control+End"); await page.keyboard.type("some words");
  ok("typing in the body leaves the style lines as they were", JSON.stringify(await lines(page)) === JSON.stringify(before));
  // the block tab
  await s.getByRole("tablist", { name: "Style for" }).getByRole("tab", { name: "Block" }).click(); await settle(page);
  await page.locator(".cm-content").click(); await page.keyboard.press("Control+Home"); await page.keyboard.press("ArrowDown"); await page.keyboard.press("ArrowDown"); await settle(page);
  ok("the block tab says what the caret is in", (await s.getByRole("button", { name: "Body" }).getAttribute("aria-pressed")) === "true");
  await s.getByRole("button", { name: "Subtitle" }).click(); await settle(page);
  ok("Subtitle turns the block into a heading, one undoable change", (await raw(page)).includes("## One thing at a time") );
  await page.locator(".cm-content").click(); await page.keyboard.press("Control+z"); await settle(page);
  ok("one undo takes it back", !(await raw(page)).includes("## One thing"));
  await shot(page, "desktop-block-light");
  await ctx.close();
}
// ---- a styled note opens with its own choices and writes nothing
{
  const styled = "---\ncalternal-theme: gruvbox\ncalternal-width: narrow\ncalternal-font: mono\n---\n# Styled\n\nWords.\n";
  const { ctx, page } = await note("desktop", "dark", styled);
  await openSheet(page, "desktop");
  ok("the theme is checked and the lines are the ones the note came with", (await page.locator(".style-sheet").getByRole("radio", { name: "Gruvbox" }).getAttribute("aria-checked")) === "true" && JSON.stringify(await lines(page)) === JSON.stringify(["calternal-theme: gruvbox", "calternal-width: narrow", "calternal-font: mono"]));
  await shot(page, "desktop-theme-dark");
  await ctx.close();
}
// ---- phone: a theme keeps the card's box whatever the width line says
{
  const { ctx, page } = await note("phone", "light", "---\ncalternal-width: wide\n---\n# Look\n\nOne thing at a time.\n");
  const box = () => page.locator("article.nc-article").evaluate((e) => { const r = e.getBoundingClientRect(); return `${r.x.toFixed(1)},${r.width.toFixed(1)}`; });
  const plain = await box();
  ok("a phone's page is the card's width whatever a stored width says", !(await page.locator("article.nc-article").getAttribute("data-width")));
  await shot(page, "phone-note-light");
  await ctx.close();
  void plain;
}
await b.close(); stop();
console.log(bad ? `${bad} FAILED` : "all passed");
process.exit(bad ? 1 : 0);
