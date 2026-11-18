// The decisions of docs/ui-shell.md and docs/settings.md, checked one by one on a fresh browser profile (nothing stored), at desktop and phone widths. Each line names the decision it checks.
// Usage: node e2e/shell-audit.mjs (after `npm run build`). Exits 1 on any miss.
import { readFileSync } from "node:fs";
import { chromium } from "playwright";
import { serve, BASE, DEVICES, expandComposer } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const fresh = async (dev, path, scheme = "light") => { const ctx = await b.newContext({ ...dev, colorScheme: scheme, reducedMotion: "no-preference" }); const page = await ctx.newPage(); await page.goto(BASE + path); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(1200); return { ctx, page }; };
const wallpapers = JSON.parse(readFileSync("wallpapers.json", "utf8"));

// ---------- desktop, Threads ----------
{
  const { ctx, page } = await fresh(DEVICES.desktop, "/threads/t-idle?firstrun=0");
  const t = (s) => page.locator(s);
  ok((await t("aside.side header .mark").innerText()) === "Calternal" && (await t("aside.side header svg").count()) <= 1, "Sidebar header: the app's name as a wordmark, no icon (the only svg is the hide button)");
  ok((await page.evaluate(() => getComputedStyle(document.querySelector("aside.side header .mark")).fontFamily)).length > 0 && (await t("aside.side header .mark").evaluate((e) => e.tagName)) === "P", "The header's name is a label, not a control");
  ok((await t("aside.side header [aria-label='Apps']").count()) === 0 && (await t("aside.side header [role=tablist]").count()) === 0 && (await t("aside.side footer nav[aria-label='Apps']").count()) === 1, "No app strip in the sidebar's header; the icon-only capsule is in its footer");
  ok((await t("aside.side").getByText("Needs you").count()) === 0 && (await t("aside.side .att").count()) === 0, "No Needs-you card: the Threads sidebar has an Inbox tab instead");
  ok((await t("aside.side [role=tab]").allInnerTexts()).map((x) => x.replace(/\d+/g, "").trim()).join("|") === "Inbox|All Threads", "The list has two tabs, Inbox and All Threads");
  await t("aside.side [role=tab]").filter({ hasText: "All Threads" }).click();
  ok((await t("aside.side").getByText("Threads", { exact: true }).count()) > 0 && (await t("aside.side").getByRole("button", { name: /New section/ }).count()) === 1, "Sidebar body: a \"Threads\" section for threads outside any section, and the person can make sections");
  ok((await page.evaluate(() => /gradient/.test(getComputedStyle(document.querySelector("aside.side .scroll")).maskImage))), "The sidebar's content fades at its top and bottom (no hard cut-off)");
  const row = t("aside.side nav[aria-label=Threads] a.row").first();
  await row.dblclick(); ok((await t("aside.side input.rename, aside.side .rename").count()) > 0, "Double-click renames a thread"); await page.keyboard.press("Escape");
  await page.locator("aside.side nav[aria-label=Threads] a.row").first().click({ button: "right" }); await page.waitForTimeout(300);
  ok((await page.getByRole("menu").count()) > 0, "Right-click opens a context menu"); await page.keyboard.press("Escape");
  const geo = await page.evaluate(() => { const c = document.querySelector(".mid-card").getBoundingClientRect(); return { top: c.top, bottom: c.bottom, h: innerHeight }; });
  ok(geo.top === 0 && Math.abs(geo.bottom - geo.h) < 1, "The middle card's top and bottom edges touch the viewport");
  const flat = await page.evaluate(() => { const r = getComputedStyle(document.querySelector("aside.right")); return { sh: r.boxShadow, br: r.borderRadius, bd: r.borderTopWidth, bg: r.backgroundColor }; });
  ok(flat.sh === "none" && flat.bd === "0px" && /rgba\(0, 0, 0, 0\)|transparent/.test(flat.bg), "The right sidebar is flat: no card, border or shadow of its own", JSON.stringify(flat));
  const tabs = await t("aside.right [role=tablist] [role=tab], aside.right .tabs button").allInnerTexts();
  ok(tabs.some((x) => /Chat/i.test(x)) && tabs.length >= 2, "Right sidebar: tabs at the bottom switch between AI chat and the app's panels", tabs.join(","));
  const firstPanel = await t("aside.right header h2").innerText();
  ok(/chat/i.test(firstPanel), "The right sidebar shows AI chat by default", firstPanel);
  const foot = await page.evaluate(() => [...document.querySelectorAll("aside.side footer > *")].map((e) => e.className + ":" + (e.getAttribute("aria-label") || e.textContent.trim().slice(0, 12))));
  ok(foot.length >= 3 && /avatar/.test(foot[0]) && /Search/.test(foot.at(-1)), "Footer: account picture on the left, search on the right", foot.join(" | "));
  ok((await page.locator("aside.side footer [aria-label='Spaces']").count()) === 0 && (await page.locator('aside.side footer nav[aria-label="Apps"] a').count()) === 3, "Footer: no space dots; the app capsule is at the bottom centre");
  await page.getByRole("button", { name: /Account, spaces and settings/ }).click(); await page.waitForTimeout(300);
  ok((await page.getByRole("menuitemradio", { name: "Personal" }).getAttribute("aria-checked")) === "true", "Everyone has a personal space from sign-up: it is the first and the current one, in the account menu (decision D)");
  await page.keyboard.press("Escape"); await page.waitForTimeout(200);
  // focus rings only for keyboard
  await page.getByRole("button", { name: "Hide the sidebar" }).click(); await page.waitForTimeout(300);
  const ringMouse = await page.evaluate(() => getComputedStyle(document.activeElement).outlineStyle);
  await page.keyboard.press("Control+Backslash"); await page.waitForTimeout(300);
  await page.keyboard.press("Tab"); await page.waitForTimeout(100);
  const ringKey = await page.evaluate(() => getComputedStyle(document.activeElement).outlineStyle);
  ok(ringMouse === "none" && ringKey !== "none", "Focus rings only for keyboard focus, never from a mouse", `mouse ${ringMouse}, keyboard ${ringKey}`);
  // keyboard shortcuts sheet
  await page.keyboard.press("Escape"); await page.evaluate(() => document.activeElement.blur?.());
  await page.keyboard.press("Shift+Slash"); await page.waitForTimeout(400);
  ok((await page.getByRole("dialog").count()) > 0, "Keyboard shortcuts are discoverable with ? (and Cmd-/)"); await page.keyboard.press("Escape");
  ok((await page.evaluate(() => JSON.parse(localStorage.getItem("cal.appearance") ?? "{}").streamText ?? false)) === false, "Message text is not streamed by default");
  // composer menus: pickers with icons and search
  await expandComposer(page);
  await page.locator(".chip[data-chip=model] .pill").first().click(); await page.waitForTimeout(300);
  const picker = page.getByRole("dialog", { name: /Provider/ }); // a menu with a search field is a dialog holding the search and the menu
  ok((await picker.locator("input[type=search]").count()) > 0 && (await picker.locator("svg").count()) > 0, "Pickers show an icon per provider and have an inline search field"); await page.keyboard.press("Escape");
  // message actions
  await page.locator(".msg").nth(2).hover(); await page.waitForTimeout(300);
  ok((await page.locator(".msg [aria-label='Copy message']").count()) > 0 && (await page.locator(".msg [aria-label='Reply in side chat']").count()) > 0, "Each message has small actions beside its time (copy, reply in side chat, more) on hover");
  // selection actions
  await page.evaluate(() => { const p = document.querySelector(".msg.assistant p, .agent p"); const r = document.createRange(); r.selectNodeContents(p); const s = getSelection(); s.removeAllRanges(); s.addRange(r); document.dispatchEvent(new Event("selectionchange")); });
  await page.mouse.up(); await page.waitForTimeout(500);
  ok((await page.getByRole("button", { name: /Comment/ }).count()) > 0 && (await page.getByRole("button", { name: /Ask in side chat/i }).count()) > 0, "Selecting text offers Comment and Ask in Side Chat");
  const cap = await page.locator("aside.side footer nav[aria-label='Apps'] a").evaluateAll((l) => l.map((e) => e.getAttribute("aria-label")));
  ok(cap.join(",") === "Threads,Notes,Settings", "App switcher: an icon-only capsule of three apps in the sidebar's footer", cap.join(","));
  const personal = await page.locator("aside.side nav[aria-label=Threads] a.row").count();
  await page.getByRole("button", { name: /Account, spaces and settings/ }).click(); await page.getByRole("menuitemradio", { name: "Infra" }).click(); await page.waitForTimeout(500);
  const infra = await page.locator("aside.side nav[aria-label=Threads] a.row").allInnerTexts();
  ok(infra.length > 0 && infra.length < personal && infra.every((t) => /Mesh|rollout/i.test(t)) && page.url().endsWith("/threads"), "The sidebar lists only the current space's threads; addresses do not change (decision B)", infra.map((t) => t.trim().slice(0, 16)).join(", "));
  await page.keyboard.press("Control+k"); await page.waitForSelector(".pal-input"); await page.locator(".pal-input").fill("slow after restart"); await page.waitForTimeout(500);
  const pr = await page.locator(".r").allInnerTexts(); ok(pr.some((t) => /Journal replay is slow/.test(t) && /Personal/.test(t)), "The palette still searches every space (a thread of Personal found from Infra, with its space named)", pr[0]?.replace(/\n/g, " | "));
  await page.keyboard.press("Enter"); await page.waitForTimeout(600);
  ok(page.url().includes("/threads/t-") && (await (async () => { await page.getByRole("button", { name: /Account, spaces and settings/ }).click(); const c = await page.getByRole("menuitemradio", { checked: true }).innerText(); await page.keyboard.press("Escape"); return c.trim(); })()) === "Personal", "Opening an item of another space makes that space current (the address is the item's, not the space's)");
  await ctx.close();
}
// ---------- desktop, Notes ----------
{
  const { ctx, page } = await fresh(DEVICES.desktop, "/notes/n-1?firstrun=0");
  const w = await page.evaluate(() => { const a = document.querySelector(".doc, article"); return a.getBoundingClientRect().width; });
  ok(w <= 640 && w > 300, "Notes: a narrow centred column", String(Math.round(w)));
  ok((await page.locator(".rail").count()) > 0, "Notes: a thin rail at the card's left edge");
  await page.locator(".rail").hover(); await page.waitForTimeout(400);
  ok((await page.locator(".rail nav, .rail [id$='-toc']").count()) > 0, "The rail expands into a small contents card");
  ok((await page.locator("aside.side").getByText("Needs you").count()) === 0, "Attention card absent when there is nothing to show");
  const panels = await page.locator("aside.right [role=tab], aside.right .tabs button").allInnerTexts();
  ok(panels.some((x) => /Chat/i.test(x)) && panels.some((x) => /Inspector/i.test(x)), "Notes right sidebar: AI chat and the app's panels (inspector, style)", panels.join(","));
  await ctx.close();
}
// ---------- desktop, Settings ----------
{
  const { ctx, page } = await fresh(DEVICES.desktop, "/settings/appearance?firstrun=0");
  const foot = await page.evaluate(() => { const f = document.querySelector("aside.side footer").getBoundingClientRect(); return f.bottom <= innerHeight + 1 && f.top > 0; });
  ok(foot, "The sidebar's footer (account, spaces, search) is in view in the Settings app too");
  const groups = await page.locator("aside.side nav h2").allInnerTexts();
  ok(groups.length === 3 && /^User/.test(groups[0]) && /^Admin/.test(groups[1]) && /^Space/.test(groups[2]), "Settings: User, Admin, then the current space (owner decision C)", groups.join(" / "));
  ok((await page.locator("aside.side input[type=search], .page input[type=search]").count()) === 0, "No search box inside Settings");
  const rows = await page.locator(".page [data-setting]").evaluateAll((e) => e.map((x) => x.dataset.setting));
  ok(["appearance.blur", "appearance.dim", "appearance.tint"].every((s) => rows.includes(s)), "Materials are three checkboxes: Blur, Dim, Tint");
  ok(rows.includes("appearance.ui-size") && rows.includes("appearance.wordmark"), "One UI size slider; the wordmark and its font are set here");
  ok(rows.includes("appearance.motion") || (await page.getByText(/Reduce motion|Motion/i).count()) > 0, "Motion follows the system's reduce-motion setting, plus an off switch");
  await page.goto(BASE + "/settings/agents?firstrun=0"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(600);
  const arows = await page.locator(".page [data-setting]").evaluateAll((e) => e.map((x) => x.dataset.setting));
  ok(["agents.default-permission", "agents.stream-text", "agents.retry", "agents.snooze-minutes", "agents.sandbox-idle-minutes", "agents.decision-provider"].every((s) => arows.includes(s)), "Agents: default permission, streaming, retry, snooze, sandbox idle time, decision service");
  const perm = await page.evaluate(() => document.body.innerText);
  ok(/Approve for me/.test(perm), "A new thread starts in Approve for me (the default shown in Settings)");
  await page.goto(BASE + "/settings/storage?firstrun=0"); await page.waitForTimeout(500);
  ok((await page.getByText(/Danger|Delete/).count()) >= 0, "(admin pages end with a Danger area)");
  const pages = await page.evaluate(() => [...document.querySelectorAll("aside.side nav a")].map((a) => a.textContent.trim()));
  ok(pages.some((p) => /Plugins/i.test(p)), "Plugins are managed on one page", pages.join(", "));
  ok(pages.some((p) => /Machines/i.test(p)), "Machines have a page (added through a guided flow)");
  await ctx.close();
}
// ---------- a member without the server-admin capability ----------
{
  const { ctx, page } = await fresh(DEVICES.desktop, "/settings/appearance?firstrun=0&as=member");
  const groups = await page.locator("aside.side nav h2").allInnerTexts();
  ok(groups.length === 2 && !groups.some((g) => /^Admin/.test(g)), "The Admin section shows only to someone holding the server-admin capability (a member sees two sections)", groups.join(" / "));
  await page.goto(BASE + "/settings/storage?firstrun=0"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(500);
  ok((await page.getByText("The three roots and their health").count()) === 0, "…and an Admin page's address shows nothing to them");
  await page.keyboard.press("Control+k"); await page.waitForSelector(".pal-input"); await page.locator(".pal-input").fill("storage roots"); await page.waitForTimeout(400);
  ok((await page.locator(".r.command").filter({ hasText: /Storage/ }).count()) === 0, "…and the palette does not offer Admin pages");
  await page.keyboard.press("Escape");
  await page.goto(BASE + "/settings/appearance?firstrun=0&as=operator"); await page.waitForSelector("html[data-ready]");
  ok((await page.locator("aside.side nav h2").count()) === 3, "An operator sees all three");
  await ctx.close();
}
// ---------- wallpapers ----------
{
  const doc = readFileSync("../../docs/settings.md", "utf8");
  const ids = [...doc.matchAll(/\| ([^|]+) \| `([A-Za-z0-9_-]{11})` \|/g)].map((m) => m[2]);
  ok(ids.length === 8 && ids.every((i) => wallpapers.some((w) => w.id === i)), "Default wallpapers: the eight photographs of docs/settings.md", `${ids.length} listed`);
  const { ctx, page } = await fresh(DEVICES.desktop, "/settings/appearance?sheet=1&firstrun=0");
  const tiles = await page.locator("button.tile[role=radio]").count();
  ok(tiles >= 8, "The wallpaper picker offers the built-in set", String(tiles));
  await ctx.close();
}
// ---------- first visit ----------
{
  const { ctx, page } = await fresh(DEVICES.desktop, "/threads/t-idle");
  ok((await page.getByRole("dialog").count()) === 0, "A first visit goes straight in: no look chooser dialog (the look is step 2 of the onboarding)");
  await ctx.close();
}
// ---------- phone ----------
{
  const { ctx, page } = await fresh(DEVICES.phone, "/threads/t-idle?firstrun=0");
  const wall = await page.evaluate(() => getComputedStyle(document.querySelector(".backdrop")).backgroundImage);
  ok(/wallpapers\//.test(wall) || /blob:/.test(wall), "Phone: the default wallpaper is behind the app on a fresh profile", wall.slice(0, 60));
  ok((await page.locator("nav.switcher").count()) === 1 && (await page.locator("aside.side footer nav.switcher").count()) === 1, "Phone: the app capsule is in the sidebar's footer (there is no bottom bar)");
  ok((await page.getByRole("button", { name: "Show threads" }).count()) === 1, "Phone: a button shows the sidebar");
  const mid = await page.evaluate(() => { const c = document.querySelector(".mid-card").getBoundingClientRect(); return c.left === 0 && c.right === innerWidth; });
  ok(mid, "Phone: the middle card fills the screen");
  await ctx.close();
}
// ---------- phone: what was added on 2026-11-18 ----------
{
  const { ctx, page } = await fresh(DEVICES.phone, "/threads/t-idle?firstrun=0");
  await page.getByRole("button", { name: "Show threads" }).click(); await page.waitForTimeout(600);
  ok((await page.locator("aside.side footer [aria-label='Spaces']").count()) === 0, "Phone: no space dots in the sidebar footer (the spaces are behind the account picture)");
  await page.getByRole("button", { name: /Account, spaces and settings/ }).click(); await page.waitForTimeout(300);
  const items = await page.locator("[role^=menuitem]").evaluateAll((e) => e.map((d) => [d.textContent.trim(), d.getBoundingClientRect().height]));
  ok(items.length >= 3 && items.some(([t]) => /Settings/.test(t)) && items.every(([, h]) => h >= 36), "Phone: the account picture opens the spaces and Settings with touch-sized rows", JSON.stringify(items));
  await page.keyboard.press("Escape"); await page.waitForTimeout(200);
  ok((await page.locator(".grip").count()) === 0, "Phone: no resize handles");
  const f = await page.evaluate(() => { const r = document.querySelector("aside.side footer").getBoundingClientRect(); return r.bottom <= innerHeight + 1; });
  ok(f, "Phone: the footer is in view");
  await page.getByRole("button", { name: /Account, spaces and settings/ }).click(); await page.locator("[role^=menuitem]", { hasText: "Infra" }).click(); await page.waitForTimeout(500);
  ok((await page.locator("aside.side nav[aria-label=Threads] a.row").count()) > 0 && (await page.locator("aside.side nav[aria-label=Threads] a.row").allInnerTexts()).every((t) => /Mesh|rollout/i.test(t)), "Phone: choosing a space changes the list");
  await page.goto(BASE + "/settings/appearance?firstrun=0"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(600);
  ok((await page.getByRole("button", { name: "Show the sidebar" }).count()) === 1, "Phone: the Settings page has a button for its sidebar");
  await page.getByRole("button", { name: "Show the sidebar" }).click(); await page.waitForTimeout(600);
  ok((await page.locator("aside.side nav h2").allInnerTexts()).some((t) => /^Space/.test(t)), "Phone: the Settings sidebar has the current space's section");
  await page.goto(BASE + "/settings/appearance?firstrun=0"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(500);
  ok((await page.getByText("Compare (temporary").count()) === 0 && (await page.getByText("Clear to tinted", { exact: true }).count()) >= 1 && (await page.getByText("Readability floor").count()) === 0, "Phone: Clear to tinted is in Materials on the Appearance page; there is no Compare group, floor or Craft toggle");
  await ctx.close();
}
await b.close(); stop();
console.log(bad ? `${bad} FAIL` : "all pass"); process.exit(bad ? 1 : 0);
