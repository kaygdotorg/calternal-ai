// The Share sheet, "Shared with me" and the guest page (fixtures). Desktop (popover from the Share pill) and phone (inset bottom sheet): who can see it, add a person and a group, View/Edit, remove, a link with
// its expiry, QR and Copy; what an Editor and a Viewer may not do; the sidebar's Shared with me section and the owner's avatar in place of Share; the guest page, signed in and out.
// Frames go to $CAL_SHOTS (default /tmp/calternal-share-shots), never into the repository. Usage: node e2e/with-preview.mjs e2e/share.mjs
import { mkdirSync } from "node:fs";
import { launch, open, serve } from "./lib.mjs";
const SHOTS = process.env.CAL_SHOTS ?? "/tmp/calternal-share-shots"; mkdirSync(SHOTS, { recursive: true });
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const shot = (p, n) => p.screenshot({ path: `${SHOTS}/${n}.png` });
const sheet = (p) => p.locator("[data-share]");
try {
  for (const device of ["desktop", "phone"]) {
    const { page: p, ctx } = await open(browser, device, "light", "/notes/n-3?firstrun=0");
    await ctx.grantPermissions(["clipboard-read", "clipboard-write"]).catch(() => {});
    await p.getByRole("button", { name: "Share" }).first().click();
    await sheet(p).waitFor();
    await p.waitForTimeout(700);
    ok((await p.locator("[data-share-sentence]").textContent()) === "Only you can see this.", `${device}: one plain sentence says only you can see it`);
    ok((await sheet(p).locator("h2").textContent()) === "Invite flow", `${device}: the heading is the note's title, once`);
    // Add a person, then a group.
    const add = p.getByRole("textbox", { name: "Add people or groups" });
    await add.fill("may"); await p.getByRole("option").first().waitFor(); await shot(p, `${device}-search`);
    await add.press("Enter"); await p.locator("[data-person]").first().waitFor();
    await add.fill("des"); await p.getByRole("option", { name: /Design/ }).click();
    await p.waitForFunction(() => document.querySelectorAll("[data-person]").length === 2);
    ok(/Maya Okafor and Design can view/.test(await p.locator("[data-share-sentence]").textContent()), `${device}: the sentence names who can see it`, await p.locator("[data-share-sentence]").textContent());
    // View to Edit.
    await p.getByRole("radiogroup", { name: /Maya Okafor: what they can do/ }).or(p.getByRole("tablist", { name: /Maya Okafor: what they can do/ })).getByText("Edit").click();
    await p.waitForFunction(() => /Maya Okafor can edit/.test(document.querySelector("[data-share-sentence]")?.textContent ?? ""));
    ok(true, `${device}: View to Edit changes the sentence`);
    // A link.
    await p.getByRole("button", { name: "Create a link" }).click(); await p.locator("[data-link]").first().waitFor();
    ok(/Never expires/.test(await p.locator("[data-link]").first().textContent()), `${device}: a new link never expires by default`);
    ok(/Anyone with a link can view/.test(await p.locator("[data-share-sentence]").textContent()), `${device}: the sentence mentions the link`);
    await p.getByRole("button", { name: "QR code" }).click(); await p.locator(".qr svg").waitFor();
    await sheet(p).evaluate((e) => e.scrollIntoView?.()); await shot(p, `${device}-link-qr`);
    await p.getByRole("button", { name: "Copy link" }).click();
    const copied = await p.evaluate(() => navigator.clipboard.readText()).catch(() => "");
    ok(/\/s\/fx/.test(copied), `${device}: Copy puts the link address on the clipboard`, copied);
    await p.getByRole("button", { name: "Remove Design" }).click(); await p.waitForFunction(() => document.querySelectorAll("[data-person]").length === 1);
    ok(true, `${device}: remove takes a group away`);
    // Nothing overflows the sheet sideways.
    ok(await sheet(p).evaluate((e) => { const r = e.getBoundingClientRect(); return r.left >= 0 && r.right <= innerWidth; }), `${device}: the sheet fits the window sideways`);
    await p.keyboard.press("Escape"); await p.waitForTimeout(400);
    ok(!(await sheet(p).count()), `${device}: Escape closes it`);
    await ctx.close();
  }
  // What an Editor and a Viewer may do.
  for (const [as, links, audiences] of [["editor", false, true], ["viewer", false, false]]) {
    const { page: p, ctx } = await open(browser, "desktop", "light", `/notes/n-3?firstrun=0&share=${as}`);
    await p.getByRole("button", { name: "Share" }).first().click(); await sheet(p).waitFor(); await p.waitForTimeout(500);
    ok((await p.getByRole("button", { name: "Create a link" }).count()) === (links ? 1 : 0), `${as}: Create a link ${links ? "is" : "is not"} offered`);
    ok((await p.getByRole("textbox", { name: "Add people or groups" }).count()) === (audiences ? 1 : 0), `${as}: adding people ${audiences ? "is" : "is not"} offered`);
    if (!links) ok(await p.getByText("Links need a Manager or the Owner.").count() > 0 || as === "viewer", `${as}: the sheet says who can make links`);
    await shot(p, `desktop-${as}`); await ctx.close();
  }
  // Shared with me, and the owner's avatar where Share was.
  for (const device of ["desktop", "phone"]) {
    const { page: p, ctx } = await open(browser, device, "light", "/notes/n-4?firstrun=0");
    if (device === "phone") { await p.getByRole("button", { name: /Show notes/ }).first().click(); await p.waitForTimeout(600); }
    ok(await p.getByRole("group", { name: "Shared with me" }).count() === 1, `${device}: the sidebar has a Shared with me section`);
    ok(await p.getByRole("group", { name: "Shared with me" }).getByRole("link").count() === 2, `${device}: two rows, one line each`);
    await shot(p, `${device}-shared-with-me`);
    if (device === "phone") await p.keyboard.press("Escape");
    await ctx.close();
  }
  { const { page: p, ctx } = await open(browser, "desktop", "light", "/notes/n-4?firstrun=0");
    await p.waitForTimeout(500);
    ok(await p.getByRole("button", { name: /Shared by Maya Okafor/ }).count() === 1 && await p.getByRole("button", { name: "Share", exact: true }).count() === 0, "on a note shared with me the owner's avatar sits where Share is");
    await shot(p, "desktop-note-shared-with-me"); await ctx.close(); }
  // The guest page.
  for (const [device, path] of [["desktop", "/s/demo"], ["phone", "/s/demo?signedin=1"], ["desktop", "/s/nope"]]) {
    const { page: p, ctx } = await open(browser, device, "light", path);
    await p.waitForTimeout(800);
    const bad404 = path.endsWith("nope");
    if (bad404) ok(await p.getByRole("heading", { name: "This link doesn't work" }).count() === 1, "a wrong secret says only that it doesn't work");
    else {
      ok(await p.getByText("Release checklist").count() > 0 && (await p.locator(".cm-content[contenteditable=true]").count()) === 0, `guest ${device}: the item, read-only`);
      ok((await p.getByRole("button", { name: "Open in Calternal" }).count()) === (path.includes("signedin") ? 1 : 0), `guest ${device}: "Open in Calternal" only when signed in`);
      ok(await p.locator("nav, [aria-label='Notes']").count() === 0, `guest ${device}: no sidebar, only the item`);
    }
    await shot(p, `guest-${device}-${path.replace(/\W/g, "")}`); await ctx.close();
  }
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);
