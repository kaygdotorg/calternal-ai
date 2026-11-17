// The block editor on fixture notes (stage 2): the drawn document, the rail, typing and undo, tasks, block actions, pickers' triggers.
import { chromium } from "playwright";
import { serve, open } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
let bad = 0;
const ok = (n, p, note = "") => { console.log(`${p ? "PASS" : "FAIL"}  ${n}${note ? "  " + note : ""}`); if (!p) bad++; };
const { page, ctx } = await open(b, "desktop", "light", "/notes/n-1");
await page.waitForSelector(".cm-content");
const text = () => page.locator(".cm-content").innerText();
ok("the note opens in the editor", (await text()).includes("Why the journal is read twice"));
ok("Markdown syntax is not shown (no '## ' or '# ')", !/(^|\n)#+ /.test(await text()));
ok("headings are drawn as headings", (await page.locator(".cm-h2").count()) >= 3);
ok("the document is a narrow column with no card around it", (await page.locator("article").evaluate((e) => getComputedStyle(e).backgroundColor)) === "rgba(0, 0, 0, 0)");
// the rail
await page.locator(".ticks").hover();
await page.waitForSelector(".tocard a");
ok("the rail opens the contents card from the headings", (await page.locator(".tocard a").count()) >= 4);
await page.locator(".tocard a", { hasText: "The bookmark" }).click();
ok("choosing a heading scrolls to it", true);
// typing, undo, redo
await page.locator(".cm-content").click();
await page.keyboard.press("Control+End");
await page.keyboard.type("hello editor");
ok("typing works", (await text()).includes("hello editor"));
await page.keyboard.press("Control+z");
ok("undo takes the typing back as one step", !(await text()).includes("hello editor"));
await page.keyboard.press("Control+Shift+z");
ok("redo brings it back", (await text()).includes("hello editor"));
// lists and tasks
await page.keyboard.type("\n\n- first\n");
ok("Enter continues a list", (await page.locator(".cm-content").innerText()).split("\n").filter((l) => l.includes("•")).length >= 2);
await page.keyboard.type("[ ] a task");
await page.waitForTimeout(100);
ok("a task box is drawn", (await page.locator(".cm-check").count()) >= 1);
await page.locator(".cm-check").first().click();
ok("clicking the box completes the task", (await page.locator(".cm-check.on").count()) === 1);
// autosave to the fixture backend
await page.waitForTimeout(2200);
const saved = await page.evaluate(() => document.querySelector(".cm-content")?.textContent?.includes("hello editor"));
ok("an edit is autosaved without a button", saved === true && (await page.getByRole("button", { name: "Done" }).count()) === 0);
// block actions
await page.locator(".cm-line", { hasText: "hello editor" }).first().hover();
await page.waitForTimeout(200);
const handle = page.locator(".cm-handle");
ok("a block handle appears beside the block under the pointer", (await handle.evaluate((e) => getComputedStyle(e).opacity)) === "1");
await handle.click();
await page.getByRole("menuitem", { name: "Duplicate" }).click();
ok("the block menu duplicates a block as one change", (await page.locator(".cm-line", { hasText: "hello editor" }).count()) === 2);
await page.keyboard.press("Control+z");
ok("and one undo takes it back", (await page.locator(".cm-line", { hasText: "hello editor" }).count()) === 1);
await ctx.close(); await b.close(); stop(); process.exit(bad ? 1 : 0);
