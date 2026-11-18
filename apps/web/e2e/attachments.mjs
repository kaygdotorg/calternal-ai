// The attachment row above the text (fixtures; the server has no upload operation yet): a + chip menu opens a file picker; files and pictures also arrive by paste and by drop; each shows its progress, becomes ready
// and can be removed; the row sits above the field; sending waits for uploads and the message carries the files. Usage: CAL_URL=... node e2e/attachments.mjs
import { launch, open, serve, expandComposer } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const PNG = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==", "base64");
try {
  for (const device of ["desktop", "phone"]) {
    console.log(`-- ${device}`);
    const { page: p, ctx } = await open(browser, device, "dark", "/threads/t-idle");
    await expandComposer(p);
    await p.getByRole("button", { name: "Add photos and files" }).click();
    ok((await p.getByRole("menuitem").allInnerTexts()).map((t) => t.trim()).join() === "Photos,Files", "the + chip opens a menu: Photos, Files");
    await p.keyboard.press("Escape");
    await p.locator('input[type=file]').setInputFiles([{ name: "plan.png", mimeType: "image/png", buffer: PNG }, { name: "notes.md", mimeType: "text/markdown", buffer: Buffer.alloc(30000, 65) }]);
    const row = p.locator(".atts");
    await row.waitFor({ timeout: 3000 });
    const rb = await row.boundingBox(), fb = await p.locator("#composer-field").boundingBox();
    ok(rb.y + rb.height <= fb.y + 2, "the row is above the text field", `row bottom ${Math.round(rb.y + rb.height)}, field top ${Math.round(fb.y)}`);
    ok((await p.locator(".att").count()) === 2 && (await p.locator(".att img").count()) === 1, "a picture shows a thumbnail, a file its icon and name");
    ok((await p.locator(".att[role=listitem] [role=progressbar]").count()) >= 1, "an upload shows its progress");
    await p.waitForFunction(() => !document.querySelector(".att [role=progressbar]"), null, { timeout: 8000 });
    ok(true, "and it becomes ready");
    // paste and drop
    await p.locator("#composer-field").evaluate((el, b64) => { const f = new File([Uint8Array.from(atob(b64), (c) => c.charCodeAt(0))], "pasted.png", { type: "image/png" }); const dt = new DataTransfer(); dt.items.add(f); el.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true, cancelable: true })); }, PNG.toString("base64"));
    await p.waitForFunction(() => document.querySelectorAll(".att").length === 3, null, { timeout: 3000 }).catch(() => {});
    ok((await p.locator(".att").count()) === 3, "a pasted picture joins the row");
    await p.locator(".composer").evaluate((el, b64) => { const f = new File([new Uint8Array(10)], "dropped.txt", { type: "text/plain" }); const dt = new DataTransfer(); dt.items.add(f); el.dispatchEvent(new DragEvent("dragover", { dataTransfer: dt, bubbles: true, cancelable: true })); el.dispatchEvent(new DragEvent("drop", { dataTransfer: dt, bubbles: true, cancelable: true })); }, "");
    await p.waitForFunction(() => document.querySelectorAll(".att").length === 4, null, { timeout: 3000 }).catch(() => {});
    ok((await p.locator(".att").count()) === 4, "a dropped file joins the row");
    await p.screenshot({ path: `/tmp/atts-${device}.png` });
    await p.getByRole("button", { name: "Remove dropped.txt" }).click();
    ok((await p.locator(".att").count()) === 3, "a remove button takes one out");
    await p.waitForFunction(() => !document.querySelector(".att [role=progressbar]"), null, { timeout: 8000 });
    await p.locator("#composer-field").fill("look at these"); await p.keyboard.press("Enter");
    await p.waitForFunction(() => [...document.querySelectorAll(".msg.user")].some((m) => /plan\.png/.test(m.textContent) && /look at these/.test(m.textContent)), null, { timeout: 6000 }).catch(() => {});
    ok(await p.evaluate(() => [...document.querySelectorAll(".msg.user")].some((m) => /plan\.png/.test(m.textContent) && /pasted\.png/.test(m.textContent))), "the sent message carries the files", "");
    ok((await p.locator(".atts").count()) === 0, "and the row is empty again");
    await ctx.close();
  }
} finally { await browser.close(); stop(); }
process.exit(bad ? 1 : 0);
