// The Threads interface at contract 14.43 in a browser over the fixture host (`?fixture=chat-next`; testing/chat-host.ts, invented people): the list with viewer-private counts and quiet marks, pins first and the
// People and Agents selectors, a conversation drawn from entries (grouping, own and agent messages with the host's marks, edited, asked by, queued, quotes with a jump and a back chip, inline Markdown, item links, date
// and system lines, the unread separator, the activity row), the reading position written only for messages that scrolled into view, per-entry menus, reply and edit through the composer, mentions limited to
// participants, drafts, range selection for calling an agent in, the consent card, the read-only notice, and the phone layout. Usage: node e2e/chat-entries.mjs [shots-directory]
import { mkdirSync } from "node:fs";
import { BASE as BASE_URL, launch, open, serve } from "./lib.mjs";
const shots = process.argv[2]; if (shots) mkdirSync(shots, { recursive: true });
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const errors = [];
const go = async (query, size) => { const r = await open(browser, `fixture=chat-next&${query}${process.env.SCHEME ? `&scheme=${process.env.SCHEME}` : ""}`, size); r.page.on("pageerror", (e) => errors.push(String(e))); return r; };
const snap = async (p, name) => { if (shots) { name = `${process.env.SCHEME ?? "light"}-${name}`; await p.waitForTimeout(500); await p.screenshot({ path: `${shots}/${name}.png` }); } };
const DESK = { width: 1280, height: 800 }, PHONE = { width: 390, height: 844 };
const log = (p, f) => p.evaluate((src) => window.__transport.log.filter(new Function("l", `return ${src}`)), f);
const openFirst = async (p) => { await p.waitForSelector("[data-strip=pins] [data-row-id=t-lunch] button", { timeout: 20000 }); await p.locator("[data-strip=pins] [data-row-id=t-lunch] button").evaluate((e) => e.click()); await p.waitForSelector("[data-entry]", { timeout: 10000 }); };

// ---- the list rows (the sidebar's own arrangement is in chat-sidebar.mjs)
{
  const { page: p } = await go("", DESK);
  await p.waitForSelector("[data-tree] button.row", { timeout: 20000 });
  ok((await p.locator("[data-tree] [data-row-id] button.row .s").count()) === 0 && (await p.locator("[data-tree] [data-row-id] button.row").first().locator("[data-mark=text]").count()) === 1, "every thread row is one line, with its time at the end");
  const mk = (id) => p.locator(`[data-row-id="${id}"] [data-mark=count]`).allTextContents();
  ok((await mk("t-p1")).join() === "4", "an unmuted thread shows its unread count in the accent, after its time", (await mk("t-p1")).join());
  ok((await mk("t-design")).join() === "2", "a muted thread (it never rings) keeps only its mention count, in the accent", (await mk("t-design")).join());
  ok((await mk("t-trip")).length === 0, "a thread with nothing unread shows no count");
  await snap(p, "next-desktop-1-list");
}

// ---- the conversation
{
  const { page: p } = await go("", DESK);
  await p.evaluate(() => { const t = window.__transport, o = t.post.bind(t); t.post = async (r, b, ...a) => (r === "item_view.read" ? new Promise(() => {}) : o(r, b, ...a)); }); // (the read mark is held back here so the separator can be looked at)
  await openFirst(p);
  ok((await p.locator("[data-entry]").count()) === 9, "the page is drawn from entries");
  ok((await p.locator("[data-marker=date]").count()) === 2 && (await p.locator("[data-marker=system]").textContent()).includes("called Codex in"), "date lines and a system line sit between them");
  ok((await p.locator("[data-anchor=e2] .eh").count()) === 1 && (await p.locator("[data-anchor=e3] .eh").count()) === 0, "two messages from one author within five minutes group under one name");
  ok((await p.locator("[data-anchor=e1]").getAttribute("data-own")) === "true" && (await p.locator("[data-anchor=e2]").getAttribute("data-own")) === null, "own messages are the host's flag, not the author's name");
  ok((await p.locator("[data-anchor=e4] [data-provider-mark]").count()) === 1 && (await p.locator("[data-anchor=e4] [data-asked-by]").textContent()) === "asked by Ada", "an agent's message carries the host's harness mark and who asked");
  ok((await p.locator("[data-anchor=e3] [data-edited]").count()) === 1 && (await p.locator("[data-anchor=e2] [data-edited]").count()) === 0, "an edited message says so");
  ok((await p.locator("[data-anchor=e1] strong").textContent()) === "Wednesday" && (await p.locator("[data-anchor=e4] code").textContent()) === "19:00", "inline Markdown: strong and code");
  const a = p.locator("[data-anchor=e4] a"); ok((await a.getAttribute("href")) === "https://example.test/menu" && /noopener/.test(await a.getAttribute("rel")), "an explicit http(s) link opens safely");
  ok(!(await p.locator("[data-anchor=e1]").textContent()).includes("**"), "no marker characters are left in the text");
  ok((await p.locator("[data-anchor=e9] [data-state-mark]").textContent()) === "Sending…", "a queued message says it is sending");
  ok(await p.locator("[data-anchor=e6] [data-quote]").evaluate((q) => { const b = getComputedStyle(q, "::before"); return b.width === "3px" && b.borderTopLeftRadius === "0px" && getComputedStyle(q).borderTopLeftRadius === "0px" && getComputedStyle(q).borderBottomLeftRadius === "0px"; }), "a quote's leading bar is a straight, square-ended strip");
  ok((await p.locator("[data-anchor=e6] [data-quote]").textContent()).includes("River it is. Booking now.") && (await p.locator("[data-anchor=e6] .qa").textContent()) === "Ada", "a reply shows the host's quoted text and author");
  ok((await p.locator("[data-activity=working]").textContent()).includes("Codex is working"), "the agent's working state shows under the last message");
  await snap(p, "next-desktop-2-conversation");
  // unread
  ok((await p.locator("[data-unread-separator]").textContent()).includes("3 unread messages"), "an unread separator with the count sits before the first unread message");
  // jump from a quote and back
  await p.locator("[data-anchor=e6] [data-quote]").click();
  await p.waitForSelector("[data-back-chip]", { timeout: 3000 }).then(() => ok(true, "jumping to a quoted message offers a way back"), () => ok(false, "jumping to a quoted message offers a way back"));
  ok(await p.locator("[data-anchor=e5].hl").count() === 1, "the quoted message is highlighted");
  // item link
  await p.locator("[data-anchor=e7] [data-item-link]").click(); await p.waitForTimeout(800);
  ok((await log(p, "l.route === 'item.locate'")).length === 1 && (await log(p, "l.route === 'item_view.open' && l.body.item === 't-lunch'")).length >= 1, "an item link goes through item.locate");
}

// ---- reading position
{
  const { page: p } = await go("", DESK);
  await openFirst(p);
  await p.waitForTimeout(2500);
  const read = await log(p, "l.route === 'item_view.read'");
  ok(read.length === 1 && read[0].body.upto === "e9" && read[0].body.window_ref.startsWith("win-") && !!read[0].body.request_id, "the reading position is written once the last message has scrolled into view", JSON.stringify(read[0]?.body));
  ok((await p.locator("[data-unread-separator]").count()) === 0 && (await p.locator("[data-jump-mention]").count()) === 0, "the separator and the mention control go when everything is read");
  const open1 = (await log(p, "l.route === 'item_view.open'"))[0].body;
  ok(open1.position === "unread" && open1.live === true, "the view first opens at the unread position", JSON.stringify(open1));
}
{ // jump to latest with a count while scrolled away
  const { page: p } = await go("", { width: 1280, height: 420 });
  await openFirst(p);
  await p.locator("[data-snapshot=stack]").first().evaluate((e) => { e.closest(".scroll").scrollTop = 0; }); await p.waitForTimeout(300);
  ok(await p.locator("[data-jump-latest]").isVisible(), "scrolled away from the end, a jump-to-latest control appears");
  ok((await p.locator("[data-jump-mention]").getAttribute("aria-label")) === "1 unread mentions" || (await p.locator("[data-jump-mention]").count()) === 1, "and a mention control with its count while a mention is unread");
  await snap(p, "next-desktop-3-jump");
}

// ---- menus, reply, edit, delete
{
  const { page: p } = await go("", DESK);
  await openFirst(p);
  await p.locator("[data-anchor=e2] [data-entry-menu]").click();
  const items2 = (await p.locator("[role=menuitem]").allTextContents()).map((t) => t.trim());
  ok(items2.join("|") === "Reply|Call an agent in from here|Select messages", "another person's message offers Reply and Call an agent in", items2.join("|"));
  await snap(p, "next-desktop-4-entry-menu");
  await p.getByRole("menuitem", { name: "Reply" }).click();
  await p.waitForSelector("[data-composer-mode=reply]", { timeout: 5000 });
  ok((await p.locator("[data-composer-mode=reply]").textContent()).includes("Replying to Ben"), "Reply opens the composer in reply mode naming who it answers");
  await snap(p, "next-desktop-5-reply");
  const box = p.locator("main textarea").first();
  await box.fill("Thanks, Ben."); await box.press("Enter");
  await p.waitForFunction(() => !document.querySelector("[data-composer-mode]"), null, { timeout: 5000 });
  const resolve = (await log(p, "l.route === 'activation.resolve' && l.body.action_id === 'send'"))[0]?.body;
  ok(resolve?.interaction?.entry_composer === "c-reply-e2" && resolve.interaction.form.composer.message === "Thanks, Ben." && !resolve.interaction.choices, "a reply sends the host's composer ref and the text, and asks for no agent when none is addressed", JSON.stringify(resolve?.interaction));
  ok((await box.inputValue()) === "", "the composer clears and leaves reply mode after ok");
  // own message: edit and delete
  await p.locator("[data-anchor=e5] [data-entry-menu]").click();
  const own = (await p.locator("[role=menuitem]").allTextContents()).map((t) => t.trim());
  ok(own.join("|") === "Reply|Edit|Delete for everyone|Call an agent in from here|Select messages", "your own message also offers Edit and Delete for everyone", own.join("|"));
  await p.getByRole("menuitem", { name: "Edit" }).click();
  await p.waitForSelector("[data-composer-mode=edit]", { timeout: 5000 });
  ok((await box.inputValue()) === "River it is. Booking now.", "Edit prefills the composer with the host's text");
  await snap(p, "next-desktop-6-edit");
  await p.locator("[data-composer-cancel]").click();
  await p.waitForFunction(() => document.querySelector("main textarea")?.value === "", null, { timeout: 2000 }).catch(() => {});
  ok((await p.locator("[data-composer-mode]").count()) === 0 && (await box.inputValue()) === "", "cancelling an edit leaves the field empty");
  await p.locator("[data-anchor=e5] [data-entry-menu]").click(); await p.getByRole("menuitem", { name: "Delete for everyone" }).click();
  await p.waitForSelector("[data-dialog=confirm]", { timeout: 5000 });
  ok((await p.locator("[data-material]").textContent()).includes("disappears for everyone"), "Delete for everyone confirms in the host's words before anything happens");
  await p.locator("[data-dialog] button", { hasText: "Cancel" }).click();
}

// ---- composer: agent toggle, mentions, drafts
{
  const { page: p } = await go("", DESK);
  await openFirst(p);
  const box = p.locator("main textarea").first();
  await box.fill("@B"); await p.waitForSelector("[data-mention-picker]", { timeout: 4000 });
  const opts = await p.locator("[data-mention-picker] button").allTextContents();
  ok(opts.length === 1 && opts[0].startsWith("Ben"), "typing @ lists only the thread's participants", opts.join("|"));
  await box.press("Enter"); // takes the highlighted option, does not send
  ok((await box.inputValue()) === "@Ben " && (await log(p, "l.route === 'activation.resolve'")).length === 0, "choosing a participant inserts the name and sends nothing");
  await box.type("are you free at 19:00?");
  await p.waitForTimeout(1200);
  const drafts = await log(p, "l.route === 'entry_composer.draft'");
  ok(drafts.length >= 1 && drafts.at(-1).body.text.startsWith("@Ben are you free") && drafts.at(-1).body.field === "message" && drafts.at(-1).body.form === "composer" && "expected" in drafts.at(-1).body, "the draft is saved for the person through the host, with its version", JSON.stringify(drafts.at(-1)?.body).slice(0, 160));
  await box.press("Enter");
  await p.waitForFunction(() => document.querySelector("main textarea")?.value === "", null, { timeout: 5000 });
  const r = (await log(p, "l.route === 'activation.resolve' && l.body.action_id === 'send'")).at(-1).body;
  ok(JSON.stringify(r.interaction.mention_choices) === '{"message":["m-ben"]}', "the mention travels as the host's choice ref, not as a typed name", JSON.stringify(r.interaction.mention_choices));
  // the chips: "Ask an agent" reveals the Agent and Run on chips; picked ahead, Send asks nothing more
  ok((await p.locator("[data-chip=harness]").count()) === 0, "the Agent and Run on chips are absent until Ask an agent is on");
  await box.fill("Please summarise."); await p.getByRole("button", { name: "Ask an agent" }).click();
  ok((await p.locator("[data-chip=harness]").count()) === 1 && await p.locator("[data-chip=placement]").isDisabled(), "with it on, an Agent chip appears and Run on waits for an agent");
  await snap(p, "next-desktop-11-chips");
  await p.locator("[data-chip=harness]").click();
  await p.waitForSelector("[data-dialog=choose] h2:text('Agent')", { timeout: 5000 });
  await p.locator("[data-dialog=choose] button[role=option]", { hasText: "Codex" }).click();
  await p.waitForFunction(() => /Desk machine/.test(document.querySelector("[data-chip=placement]")?.textContent ?? ""), null, { timeout: 5000 }).then(() => ok(true, "the agent chip shows the pick and Run on shows the host's default"), () => ok(false, "the agent chip shows the pick and Run on shows the host's default"));
  await box.press("Enter");
  await p.waitForSelector("[data-dialog=confirm]", { timeout: 5000 });
  const cr = (await log(p, "l.route === 'activation.resolve' && l.body.action_id === 'send'")).at(-1).body;
  ok(cr.interaction.choices.harness === "h-codex" && cr.interaction.choices.placement === "desk" && (await log(p, "l.route === 'choice.options' && l.body.slot === 'placement'")).every((l) => l.body.harness === "harness-codex" && !!l.body.activation_id), "Send uses the chip picks and asks nothing more; the placement was fetched for that agent", JSON.stringify(cr.interaction.choices));
  ok((await log(p, "l.route === 'choice.options' && l.body.slot === 'placement'")).at(-1).body.activation_id === cr.activation_id, "and for the same activation id");
  await p.locator("[data-dialog] button", { hasText: "Cancel" }).click();
  // attachments
  await p.setInputFiles("[data-file-input=files]", { name: "notes.txt", mimeType: "text/plain", buffer: Buffer.from("hello from the fixture") });
  await p.waitForSelector("[data-upload=done]", { timeout: 5000 }).then(() => ok(true, "a chosen file uploads (begin, chunks, finish) and shows as a removable chip"), () => ok(false, "a chosen file uploads (begin, chunks, finish) and shows as a removable chip"));
  const up = (await log(p, "l.route.startsWith('attachment.')")).map((l) => l.route).join();
  ok(up === "attachment.begin,attachment.chunk,attachment.finish", "in that order", up);
  const beg = (await log(p, "l.route === 'attachment.begin'"))[0].body;
  ok(beg.field === "files" && beg.form === "composer" && /^[a-f0-9]{64}$/.test(beg.sha256) && beg.bytes === 22, "the host is told the name, size and digest", JSON.stringify(beg).slice(0, 160));
  await snap(p, "next-desktop-12-attached");
  await p.waitForTimeout(300);
}

// ---- range selection and calling an agent in
{
  const { page: p } = await go("", DESK);
  await openFirst(p);
  await p.locator("[data-anchor=e2] [data-entry-menu]").click(); await p.getByRole("menuitem", { name: "Select messages" }).click();
  ok(await p.locator("[data-range-action=call-range]").isDisabled(), "Select messages in the message menu starts a selection; the range action waits for it");
  await p.waitForTimeout(400); await p.locator("[data-anchor=e4]").click(); await p.waitForTimeout(500);
  const sel = (await log(p, "l.route === 'item_view.selection'")).at(-1)?.body;
  ok(sel?.first === "e2" && sel.last === "e4" && sel.window_ref.startsWith("win-"), "two taps pick a range; the host issues the selection ref", JSON.stringify(sel));
  ok((await p.locator("[data-anchor=e3].picked").count()) === 1, "every message in the range is marked");
  await snap(p, "next-desktop-7-range");
  await p.locator("[data-range-action=call-range]").click();
  await p.waitForSelector("[data-dialog=choose] h2:text('Agent')", { timeout: 5000 });
  await p.locator("[data-dialog=choose] button[role=option]").first().click();
  await p.waitForSelector("[data-dialog=choose] h2:text('Run on')", { timeout: 5000 });
  await p.locator("[data-dialog=choose] button[role=option]").first().click();
  await p.waitForSelector("[data-dialog=confirm]", { timeout: 5000 });
  const rs = (await log(p, "l.route === 'activation.resolve' && l.body.action_id === 'call-range'"))[0]?.body;
  ok(rs?.interaction?.selection_ref === "sel-e2-e4" && rs.activation_context === "ctx-call-range", "the call carries only the host's selection ref and context", JSON.stringify(rs?.interaction));
  await snap(p, "next-desktop-8-call-confirm");
  await p.locator("[data-dialog] button", { hasText: "Cancel" }).click();
}

// ---- keyboard and screen reader: the log is one tab stop; arrows move between messages; Shift extends a selection
{
  const { page: p } = await go("", DESK);
  await openFirst(p);
  await p.waitForTimeout(300);
  await p.locator("[role=log]").focus();
  await p.keyboard.press("End"); await p.waitForTimeout(100);
  ok((await p.evaluate(() => document.activeElement?.getAttribute("data-anchor"))) === "e9", "End goes to the last message");
  ok(/^Ben, .*sending/.test((await p.locator("[data-anchor=e9]").getAttribute("aria-label")) ?? ""), "each message is named for a screen reader: who, when, state", await p.locator("[data-anchor=e9]").getAttribute("aria-label"));
  await p.keyboard.press("ArrowUp"); await p.keyboard.press("ArrowUp");
  ok((await p.evaluate(() => document.activeElement?.getAttribute("data-anchor"))) === "e7", "Up moves one message at a time");
  await p.keyboard.press("Shift+ArrowUp"); await p.keyboard.press("Shift+ArrowUp"); await p.waitForTimeout(500);
  ok((await p.locator("[role=listbox]").count()) === 1 && (await p.locator("[data-anchor=e5]").getAttribute("aria-selected")) === "true" && (await p.locator("[data-anchor=e9]").getAttribute("aria-selected")) === "false", "Shift with an arrow starts a selection: the log becomes a multi-select list and selected messages say so");
  ok((await p.locator("[data-selection-bar] .cnt").textContent()).includes("3 messages selected"), "and the count is announced", await p.locator("[data-selection-bar] .cnt").textContent());
  const ks = (await log(p, "l.route === 'item_view.selection'")).at(-1)?.body;
  ok(ks?.first === "e5" && ks.last === "e7", "the host issues the reference for exactly that range", JSON.stringify(ks));
  await p.keyboard.press("Escape"); await p.waitForTimeout(200);
  ok((await p.locator("[role=log]").count()) === 1 && (await p.locator("[data-selection-bar]").count()) === 0, "Escape ends it");
  await p.keyboard.press("Enter");
  ok((await p.locator("[role=menuitem]").count()) >= 3, "Enter on a message opens its menu from the keyboard");
  await p.keyboard.press("Escape");
}

// ---- touch: holding a message opens its menu
{
  const ctx = await browser.newContext({ viewport: PHONE, hasTouch: true, isMobile: true });
  const p = await ctx.newPage(); p.on("pageerror", (e) => errors.push(String(e)));
  const cdp = await ctx.newCDPSession(p);
  await p.goto(`${BASE_URL}/?fixture=chat-next${process.env.SCHEME ? `&scheme=${process.env.SCHEME}` : ""}`);
  await openFirst(p); await p.waitForTimeout(300);
  const box = await p.locator("[data-anchor=e5] .bub").boundingBox();
  const pt = [{ x: box.x + box.width / 2, y: box.y + box.height / 2 }];
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: pt });
  await p.waitForTimeout(250);
  ok((await p.locator("[role=menuitem]").count()) === 0, "a short touch opens no menu");
  await p.waitForTimeout(400);
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await p.waitForSelector("[role=menuitem]", { timeout: 3000 }).then(() => ok(true, "holding a message opens its menu"), () => ok(false, "holding a message opens its menu"));
  await snap(p, "next-phone-5-longpress");
  await p.keyboard.press("Escape"); await p.waitForTimeout(300);
  const b2 = await p.locator("[data-anchor=e2] .bub").boundingBox();
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: b2.x + 20, y: b2.y + 10 }] });
  await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: b2.x + 20, y: b2.y + 60 }] });
  await p.waitForTimeout(600);
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  ok((await p.locator("[role=menuitem]").count()) === 0, "moving the finger (a scroll) cancels the hold");
  await ctx.close();
}

// ---- motion: arrival, the separator leaving, the consent card resolving, jump to latest
{
  const { page: p } = await go("", DESK);
  await p.evaluate(() => { window.__arr = 0; window.__gone = 0; new MutationObserver((ms) => { for (const m of ms) { if (m.target.classList?.contains("arrive") || [...m.addedNodes].some((n) => n.classList?.contains("arrive"))) window.__arr++; if (m.target.classList?.contains("gone")) window.__gone++; } }).observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ["class"] }); });
  await openFirst(p);
  await p.waitForFunction(() => window.__gone > 0, null, { timeout: 8000 }).then(() => ok(true, "once read, the unread separator leaves (fades and gathers its height) instead of vanishing"), () => ok(false, "once read, the unread separator leaves (fades and gathers its height) instead of vanishing"));
  await p.waitForTimeout(800); ok((await p.locator("[data-unread-separator]").count()) === 0, "and is gone after the motion");
  await p.waitForFunction(() => window.__arr > 0, null, { timeout: 12000 }).then(() => ok(true, "a message that arrives while you read eases in"), () => ok(false, "a message that arrives while you read eases in"));
  const anim = await p.evaluate(() => { const s = getComputedStyle(document.documentElement); return { base: s.getPropertyValue("--dur-base").trim(), slow: s.getPropertyValue("--dur-slow").trim() }; });
  ok(/^\.?\d+(\.\d+)?m?s$/.test(anim.base) && /^\.?\d+(\.\d+)?m?s$/.test(anim.slow), "the durations are the app's tokens", JSON.stringify(anim));
}
{
  const { page: p } = await go("", { width: 1280, height: 420 });
  await openFirst(p);
  await p.locator("[role=log]").evaluate((e) => { e.scrollTop = 0; }); await p.waitForTimeout(300);
  await p.locator("[data-jump-latest]").click(); await p.waitForTimeout(60);
  const mid = await p.locator("[role=log]").evaluate((e) => (e.scrollHeight - e.clientHeight - e.scrollTop) / (e.scrollHeight - e.clientHeight));
  ok(mid > 0.02 && mid < 0.98, "jump to latest glides rather than cuts", `still ${(mid * 100).toFixed(0)}% from the end after 60 ms`);
  await p.waitForTimeout(500);
  ok(await p.locator("[role=log]").evaluate((e) => e.scrollHeight - e.clientHeight - e.scrollTop < 2), "and arrives");
  const rctx = await browser.newContext({ viewport: { width: 1280, height: 420 }, reducedMotion: "reduce" });
  const q = await rctx.newPage(); await q.goto(`${BASE_URL}/?fixture=chat-next`); await openFirst(q);
  await q.locator("[role=log]").evaluate((e) => { e.scrollTop = 0; }); await q.waitForTimeout(300);
  await q.locator("[data-jump-latest]").click(); await q.waitForTimeout(80);
  ok(await q.locator("[role=log]").evaluate((e) => e.scrollHeight - e.clientHeight - e.scrollTop < 2), "under reduced motion it is instant");
  await rctx.close();
}

// ---- mute prompt with literal options, the panel
{
  const { page: p } = await go("", DESK);
  await openFirst(p);
  ok((await p.locator("[data-panel=info]").textContent()).includes("Last activity"), "the right panel draws content nodes");
  ok(!/\d{4}-\d{2}-\d{2}T/.test(await p.locator("[data-panel=info]").textContent()) && /\d{1,2}:\d{2}/.test(await p.locator("[data-panel=info]").textContent()), "a time in the panel is shown in local time, not as stored (UTC)", (await p.locator("[data-panel=info]").textContent()).replace(/\s+/g, " ").slice(0, 80));
  ok((await p.locator("[data-tree] [data-row-id] button.row [data-mark=text]").first().evaluate((e) => e.getBoundingClientRect().width / e.closest("button.row").getBoundingClientRect().width)) <= 0.21, "the time takes about a fifth of a row at most, so the title keeps its room");
  await p.getByRole("button", { name: "Mute this chat" }).click();
  await p.waitForSelector("[data-dialog=choose]", { timeout: 5000 });
  const labels = await p.locator("[data-dialog=choose] button[role=option]").allTextContents();
  ok(labels.length === 5 && labels[0] === "1 hour", "mute offers the description's literal choices", labels.join("|"));
  await p.locator("[data-dialog=choose] button[role=option]", { hasText: "8 hours" }).click(); await p.waitForTimeout(500);
  ok((await log(p, "l.route === 'activation.resolve' && l.body.action_id === 'mute-chat'"))[0]?.body.interaction.choices.mute === "mute-8h", "the pick travels as a host-issued ref");
}

// ---- consent, read-only, loading states
{
  const { page: p } = await go("scenario=consent", DESK);
  await openFirst(p);
  const card = p.locator("[data-entry-request]").first();
  ok((await card.textContent()).includes("asks Codex to read your messages 1 to 3") && /Without an answer in \d+ s/.test(await card.textContent()), "the person whose messages are selected sees the request with a countdown");
  const btns = await card.locator("button").allTextContents();
  ok(btns.join("|") === "Allow once|Always allow in this chat|Decline", "only the decisions the host offered", btns.join("|"));
  await snap(p, "next-desktop-9-consent");
  await card.getByRole("button", { name: "Decline" }).click(); await p.waitForTimeout(500);
  const rr = (await log(p, "l.route === 'request.resolve'"))[0]?.body;
  ok(rr?.decision === "deny" && rr.card_context === "ctx-card" && !("lifetime" in rr), "Decline goes to the host with the card's own context", JSON.stringify(rr));
  ok(await p.locator("[data-request-result]").textContent().then((t) => t.includes("declined")), "and the card says what happened");
  ok((await p.locator("[data-entry-request] .fold.shut").count()) === 1, "the decisions gather away as the outcome rises in (one motion)");
}
{
  const { page: p } = await go("scenario=lapsed", DESK);
  await openFirst(p);
  ok((await p.locator("[data-access=read_only]").getAttribute("data-reason")) === "lapsed" && (await p.locator("[data-access=read_only]").textContent()).includes("no longer share a group"), "a thread whose people no longer share a group says it is read-only and why");
  ok(await p.locator("main textarea").first().isDisabled() && (await p.getByRole("button", { name: "Send" }).getAttribute("aria-disabled")) === "true", "and its composer is disabled");
  ok((await p.locator("[data-anchor=e2] [data-entry-menu]").count()) === 1, "read actions stay");
  await snap(p, "next-desktop-10-readonly");
}

// ---- the connection drops while a run continues: reconnecting, never failed
{
  const { page: p } = await go("scenario=reconnect", DESK);
  await openFirst(p);
  const n0 = await p.locator("[data-entry]").count();
  await p.waitForSelector("[data-view-state=reconnecting]", { timeout: 8000 }).then(() => ok(true, "when the connection drops the thread says it is reconnecting"), () => ok(false, "when the connection drops the thread says it is reconnecting"));
  ok((await p.locator("[data-entry]").count()) === n0 && (await p.locator("[data-view-state=error]").count()) === 0 && (await p.locator("[data-activity=working]").count()) === 1, "the messages stay, the run still shows as working, nothing shows as failed");
  ok((await p.locator("[data-view-state=reconnecting]").textContent()).includes("agent keeps running"), "in plain words: the agent keeps running and the thread catches up");
  await snap(p, "desktop-13-reconnecting");
  await p.waitForFunction(() => !document.querySelector("[data-view-state=reconnecting]") && document.querySelectorAll("[data-entry]").length > 9, null, { timeout: 20000 }).then(() => ok(true, "when the host answers again the note goes and what the run wrote meanwhile arrives"), () => ok(false, "when the host answers again the note goes and what the run wrote meanwhile arrives"));
  ok((await p.locator("[data-view-state=error]").count()) === 0, "and it never showed an error");
}

// ---- phone
{
  const { page: p } = await go("", PHONE);
  await p.waitForSelector("[data-tree] button.row", { timeout: 20000 });
  await snap(p, "next-phone-1-list");
  await openFirst(p);
  const w = await p.evaluate(() => ({ s: document.documentElement.scrollWidth, c: document.documentElement.clientWidth }));
  ok(w.s <= w.c, "nothing overflows the phone's width", JSON.stringify(w));
  const ha = p.locator("[data-action=call-in]"); ok((await ha.textContent()).trim() === "" && (await ha.getAttribute("aria-label")) === "Call an agent in" && (await p.locator("main h1").textContent()).includes("Where to eat on Friday"), "on a phone the header action is its icon only (named for a screen reader) and the title is whole", await p.locator("main h1").textContent());
  await snap(p, "next-phone-2-conversation");
  await p.locator("[data-anchor=e5] [data-entry-menu]").click(); await p.waitForSelector("[role=menuitem]", { timeout: 3000 });
  const mb = await p.locator("[role=menuitem]").first().boundingBox(); ok(mb.height >= 44, "menu rows are 44 pt tall", String(mb.height));
  await snap(p, "next-phone-3-menu");
  await p.keyboard.press("Escape");
  await p.getByRole("menuitem").first().waitFor({ state: "detached", timeout: 3000 }).catch(() => {});
  await p.locator("[data-anchor=e2] [data-entry-menu]").click(); await p.getByRole("menuitem", { name: "Reply" }).click();
  await p.waitForSelector("[data-composer-mode=reply]"); await snap(p, "next-phone-4-reply");
}
ok(errors.length === 0, "no script error", errors[0]);
await browser.close(); stop();
process.exit(bad ? 1 : 0);
