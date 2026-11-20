// The Threads interface at contract 14.43 in a browser over the fixture host (`?fixture=threads-next`; testing/threads-host.ts, invented people): the list with viewer-private counts and quiet marks, pins first and the
// People and Agents selectors, a conversation drawn from entries (grouping, own and agent messages with the host's marks, edited, asked by, queued, quotes with a jump and a back chip, inline Markdown, item links, date
// and system lines, the unread separator, the activity row), the reading position written only for messages that scrolled into view, per-entry menus, reply and edit through the composer, mentions limited to
// participants, drafts, range selection for calling an agent in, the consent card, the read-only notice, and the phone layout. Usage: node e2e/threads-entries.mjs [shots-directory]
import { mkdirSync } from "node:fs";
import { launch, open, serve } from "./lib.mjs";
const shots = process.argv[2]; if (shots) mkdirSync(shots, { recursive: true });
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const errors = [];
const go = async (query, size) => { const r = await open(browser, `fixture=threads-next&${query}`, size); r.page.on("pageerror", (e) => errors.push(String(e))); return r; };
const snap = async (p, name) => { if (shots) { await p.waitForTimeout(500); await p.screenshot({ path: `${shots}/${name}.png` }); } };
const DESK = { width: 1280, height: 800 }, PHONE = { width: 390, height: 844 };
const log = (p, f) => p.evaluate((src) => window.__transport.log.filter(new Function("l", `return ${src}`)), f);
const openFirst = async (p) => { await p.waitForSelector("[data-i] button.row", { timeout: 20000 }); await p.locator("[data-i] button.row").first().evaluate((e) => e.click()); await p.waitForSelector("[data-entry]", { timeout: 10000 }); };

// ---- the list
{
  const { page: p } = await go("", DESK);
  await p.waitForSelector("[data-i] button.row", { timeout: 20000 });
  const rows = await p.locator("[data-i] button.row").evaluateAll((els) => els.map((e) => e.textContent.trim()));
  ok(rows.length === 7 && rows[0].startsWith("Where to eat") && rows[1].startsWith("Release notes"), "pinned threads come first, then the rest by activity", rows.slice(0, 2).join(" | "));
  ok(/Draft: Add the new search/.test(rows[1]), "a thread with a draft shows it as its last line");
  const marks = await p.locator("[data-i] button.row").evaluateAll((els) => els.map((e) => [...e.querySelectorAll("[data-mark=count]")].map((m) => `${m.textContent}${m.classList.contains("quiet") ? "q" : ""}`)));
  ok(marks[0].join() === "3", "an unmuted thread shows its unread count in the accent", marks[0].join());
  ok(marks[3].join() === "2,41q" || marks[3].join() === "2,41q".replace(",", ""), "a muted thread keeps a mention count in the accent and a quiet unread count at the end", marks[3].join());
  ok(marks[1].length === 0, "a thread with nothing unread shows no count");
  await snap(p, "next-desktop-1-list");
  await p.getByText("People", { exact: true }).first().click(); await p.waitForTimeout(600);
  ok((await p.locator("[data-i] button.row").count()) === 3, "People shows only the threads with people in them");
  await p.getByText("Agents", { exact: true }).first().click(); await p.waitForTimeout(600);
  ok((await p.locator("[data-i] button.row").count()) === 3, "Agents shows only the threads with agents alone");
  await p.getByText("All", { exact: true }).first().click(); await p.waitForTimeout(600);
}

// ---- the conversation
{
  const { page: p } = await go("", DESK);
  await openFirst(p);
  ok((await p.locator("[data-entry]").count()) === 10, "the page is drawn from entries");
  ok((await p.locator("[data-marker=date]").count()) === 2 && (await p.locator("[data-marker=system]").textContent()).includes("called Codex in"), "date lines and a system line sit between them");
  ok((await p.locator("[data-anchor=e2] .eh").count()) === 1 && (await p.locator("[data-anchor=e3] .eh").count()) === 0, "two messages from one author within five minutes group under one name");
  ok((await p.locator("[data-anchor=e1]").getAttribute("data-own")) === "true" && (await p.locator("[data-anchor=e2]").getAttribute("data-own")) === null, "own messages are the host's flag, not the author's name");
  ok((await p.locator("[data-anchor=e4] [data-provider-mark]").count()) === 1 && (await p.locator("[data-anchor=e4] [data-asked-by]").textContent()) === "asked by Ada", "an agent's message carries the host's harness mark and who asked");
  ok((await p.locator("[data-anchor=e3] [data-edited]").count()) === 1 && (await p.locator("[data-anchor=e2] [data-edited]").count()) === 0, "an edited message says so");
  ok((await p.locator("[data-anchor=e1] strong").textContent()) === "Wednesday" && (await p.locator("[data-anchor=e4] code").textContent()) === "19:00", "inline Markdown: strong and code");
  const a = p.locator("[data-anchor=e4] a"); ok((await a.getAttribute("href")) === "https://example.test/menu" && /noopener/.test(await a.getAttribute("rel")), "an explicit http(s) link opens safely");
  ok(!(await p.locator("[data-anchor=e1]").textContent()).includes("**"), "no marker characters are left in the text");
  ok((await p.locator("[data-anchor=e10] [data-state-mark]").textContent()) === "Sending…", "a queued message says it is sending");
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
  await p.locator("[data-anchor=e8] [data-item-link]").click(); await p.waitForTimeout(800);
  ok((await log(p, "l.route === 'item.locate'")).length === 1 && (await log(p, "l.route === 'item_view.open' && l.body.item === 'r-0'")).length >= 1, "an item link goes through item.locate");
}

// ---- reading position
{
  const { page: p } = await go("", DESK);
  await openFirst(p);
  await p.waitForTimeout(2500);
  const read = await log(p, "l.route === 'item_view.read'");
  ok(read.length === 1 && read[0].body.upto === "e10" && read[0].body.window_ref.startsWith("win-") && !!read[0].body.request_id, "the reading position is written once the last message has scrolled into view", JSON.stringify(read[0]?.body));
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
  ok(items2.join("|") === "Reply|Call an agent in from here", "another person's message offers Reply and Call an agent in", items2.join("|"));
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
  ok(own.join("|") === "Reply|Edit|Delete for everyone|Call an agent in from here", "your own message also offers Edit and Delete for everyone", own.join("|"));
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
  // the agent toggle reveals the harness and placement choices only when it is on
  await box.fill("Please summarise."); await p.getByLabel("Ask an agent").check();
  await box.press("Enter");
  await p.waitForSelector("[data-dialog=choose] h2:text('Agent')", { timeout: 5000 }).then(() => ok(true, "with Ask an agent on, Send asks which agent"), () => ok(false, "with Ask an agent on, Send asks which agent"));
  await p.locator("[data-dialog=choose] button[role=option]").first().click();
  await p.waitForSelector("[data-dialog=choose] h2:text('Run on')", { timeout: 5000 });
  await p.locator("[data-dialog] button", { hasText: "Cancel" }).click().catch(() => {});
  await p.waitForTimeout(300);
}

// ---- range selection and calling an agent in
{
  const { page: p } = await go("", DESK);
  await openFirst(p);
  await p.locator("[data-select-start]").click();
  ok(await p.locator("[data-range-action=call-range]").isDisabled(), "the range action waits for a selection");
  await p.locator("[data-anchor=e2]").click(); await p.locator("[data-anchor=e4]").click(); await p.waitForTimeout(500);
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

// ---- mute prompt with literal options, the panel
{
  const { page: p } = await go("", DESK);
  await openFirst(p);
  ok((await p.locator("[data-panel=info]").textContent()).includes("Last activity"), "the right panel draws content nodes");
  await p.getByRole("button", { name: "Mute this thread" }).click();
  await p.waitForSelector("[data-dialog=choose]", { timeout: 5000 });
  const labels = await p.locator("[data-dialog=choose] button[role=option]").allTextContents();
  ok(labels.length === 5 && labels[0] === "1 hour", "mute offers the description's literal choices", labels.join("|"));
  await p.locator("[data-dialog=choose] button[role=option]", { hasText: "8 hours" }).click(); await p.waitForTimeout(500);
  ok((await log(p, "l.route === 'activation.resolve' && l.body.action_id === 'mute-thread'"))[0]?.body.interaction.choices.mute === "mute-8h", "the pick travels as a host-issued ref");
}

// ---- consent, read-only, loading states
{
  const { page: p } = await go("scenario=consent", DESK);
  await openFirst(p);
  const card = p.locator("[data-anchor=e9] ~ [data-entry-request], [data-entry-request]").first();
  ok((await card.textContent()).includes("asks Codex to read your messages 1 to 3") && /Without an answer in \d+ s/.test(await card.textContent()), "the person whose messages are selected sees the request with a countdown");
  const btns = await card.locator("button").allTextContents();
  ok(btns.join("|") === "Allow once|Always allow in this thread|Decline", "only the decisions the host offered", btns.join("|"));
  await snap(p, "next-desktop-9-consent");
  await card.getByRole("button", { name: "Decline" }).click(); await p.waitForTimeout(500);
  const rr = (await log(p, "l.route === 'request.resolve'"))[0]?.body;
  ok(rr?.decision === "deny" && rr.card_context === "ctx-card" && !("lifetime" in rr), "Decline goes to the host with the card's own context", JSON.stringify(rr));
  ok(await p.locator("[data-request-result]").textContent().then((t) => t.includes("declined")), "and the card says what happened");
}
{
  const { page: p } = await go("scenario=lapsed", DESK);
  await openFirst(p);
  ok((await p.locator("[data-access=read_only]").getAttribute("data-reason")) === "lapsed" && (await p.locator("[data-access=read_only]").textContent()).includes("no longer share a group"), "a thread whose people no longer share a group says it is read-only and why");
  ok(await p.locator("main textarea").first().isDisabled() && (await p.getByRole("button", { name: "Send" }).getAttribute("aria-disabled")) === "true", "and its composer is disabled");
  ok((await p.locator("[data-anchor=e2] [data-entry-menu]").count()) === 1, "read actions stay");
  await snap(p, "next-desktop-10-readonly");
}

// ---- phone
{
  const { page: p } = await go("", PHONE);
  await p.waitForSelector("[data-i] button.row", { timeout: 20000 });
  await snap(p, "next-phone-1-list");
  await p.locator("[data-i] button.row").first().evaluate((e) => e.click()); await p.waitForSelector("[data-entry]", { timeout: 10000 });
  const w = await p.evaluate(() => ({ s: document.documentElement.scrollWidth, c: document.documentElement.clientWidth }));
  ok(w.s <= w.c, "nothing overflows the phone's width", JSON.stringify(w));
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
