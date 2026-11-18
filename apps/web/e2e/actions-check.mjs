// Every message action does what it says, one check for fixtures and for the server (shared by message-actions.mjs and steps-server.mjs): hover opens the actions beside the time (an agent's before its time, your own
// after it); Copy puts the message on the clipboard; Reply in side chat shows the side chat with the message quoted; Quote in the message box quotes it in the composer; Copy link copies the message's address.
/** Runs the checks on `page` (an open thread with at least one agent message), reporting through `ok`. `device` is "desktop" or "phone". */
/** Points at a message the way a person does (from outside it) and waits until its actions have opened beside the time. */
async function point(page, msg, device) {
  await msg.evaluate((e) => e.scrollIntoView({ block: "center" })); // not under the composer
  if (device !== "desktop") return;
  await page.mouse.move(2, 2);
  const b = await msg.boundingBox();
  await page.mouse.move(b.x + 120, b.y + 10); await page.mouse.move(b.x + 140, b.y + 14);
  await msg.locator(".acts").evaluate((a) => new Promise((res) => { const t0 = performance.now(); const f = () => (a.getBoundingClientRect().width > 40 || performance.now() - t0 > 3000 ? res() : requestAnimationFrame(f)); f(); }));
}

export async function checkMessageActions(page, ok, device) {
  const msg = page.locator(".msg.agent-row").last();
  await point(page, msg, device);
  const wantText = (await msg.locator(".selectable, .md").first().innerText()).trim().slice(0, 40);
  const box = await msg.boundingBox();
  if (device === "desktop") {
    await page.mouse.move(box.x + 120, box.y + 10); await page.mouse.move(box.x + 140, box.y + 14);
    await page.waitForFunction((sel) => { const a = document.querySelector(sel); return a && getComputedStyle(a).opacity === "1" && a.getBoundingClientRect().width > 40; }, ".msg.agent-row:last-of-type .acts", { timeout: 4000 }).catch(() => {});
    const order = await msg.locator(".meta").evaluate((m) => [...m.children].map((c) => c.className.split(" ")[0]));
    ok(order[0] === "acts" && order[1] === "t", "an agent message's actions come before its time", order.join(","));
    const user = page.locator(".msg.user").last();
    const uo = await user.locator(".meta").evaluate((m) => [...m.children].map((c) => c.className.split(" ")[0])).catch(() => []);
    ok(uo[0] === "t" && uo[1] === "acts", "your own message's actions come after its time", uo.join(","));
  }
  const act = (name) => msg.getByRole("button", { name });
  const reply = async () => {
  await point(page, msg, device);
    await act("Reply in side chat").click();
    await page.locator("#side-field").waitFor({ state: "visible", timeout: 4000 }).catch(() => {});
    await page.waitForFunction(() => document.querySelector("#side-field")?.value?.startsWith("> "), null, { timeout: 4000 }).catch(() => {});
    const side = await page.locator("#side-field").inputValue().catch(() => "");
    ok(side.startsWith("> "), "Reply in side chat shows the side chat with the message quoted", side.slice(0, 30));
  };
  // On a desktop the side panel shows beside the thread, so reply first; on a phone it covers the thread, so reply last.
  if (device === "desktop") await reply();
  // Copy
  await act("Copy message").click();
  const clip = await page.evaluate(() => navigator.clipboard.readText()).catch(() => "");
  ok(clip.startsWith(wantText.slice(0, 20)), "Copy message puts the message on the clipboard", clip.slice(0, 30));
  // Copy link
  await point(page, msg, device);
  await act("Message actions").click();
  await page.getByRole("menuitem", { name: "Copy link to this message" }).click();
  await page.waitForTimeout(300);
  const link = await page.evaluate(() => navigator.clipboard.readText()).catch(() => "");
  const id = await msg.locator("div.agent[id]").getAttribute("id");
  ok(!!id && link.endsWith(`#${id}`), "Copy link copies the message's address", link.slice(-50));
  // Quote in the message box
  await point(page, msg, device);
  await act("Message actions").click();
  await page.getByRole("menuitem", { name: "Quote in the message box" }).click();
  const comp = await page.locator("#composer-field").inputValue().catch(async () => await page.locator("#composer-field").innerText());
  ok(comp.startsWith("> "), "Quote in the message box puts it in the composer as a quote", comp.slice(0, 30));
  await page.getByRole("menuitem").first().waitFor({ state: "detached", timeout: 4000 }).catch(() => {}); // the menu has finished closing
  if (device !== "desktop") await reply();
}
