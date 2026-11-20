import assert from "node:assert/strict";
import test, { after } from "node:test";
import { decode } from "../src/format.ts";
import { FakeHost, handlers, peopleFromBundle, checkTrace } from "./host.ts";

after(checkTrace);

const alice = "019a0000-0000-7000-8000-000000000001";
const bob = "019a0000-0000-7000-8000-000000000002";
const cara = "019a0000-0000-7000-8000-000000000003";

async function setup() {
  const host = new FakeHost();
  const people = peopleFromBundle();
  const ctx = (actor: string, req: string) => host.peopleContext(actor, req);
  const { created: item } = await people.create({ title: "Neighbours", participants: [bob, cara] }, ctx(alice, "create"));
  const post = (actor: string, request: string, text: string) => people.post({ item, text }, ctx(actor, request));
  const get = (actor: string, message: string) => people.message({ item, message }, ctx(actor, "read"));
  const first = await post(alice, "a1", "Alice visible");
  const second = await post(bob, "b1", "Bob private");
  const third = await post(cara, "c1", "Cara visible");
  return { host, people, ctx, post, get, item, first, second, third };
}

test("a people-only thread keeps message anchors stable across authors and replay", async () => {
  const { host, item, first, second, post, get } = await setup();
  assert.deepEqual(decode(host.markdown(item)).participants.map(({ kind, id }) => ({ kind, id })), [alice, bob, cara].map(id => ({ kind: "person", id })));
  assert.notEqual(first.message, second.message);
  assert.deepEqual(await post(alice, "a1", "Alice visible"), first);
  assert.equal((await get(bob, first.message)).text, "Alice visible");
  assert.ok(host.markdown(item).includes(`{#m-${first.message} `));
  await assert.rejects(get("outsider", first.message), /unavailable/i);
});

test("the shared write attests to its sender and edit and tombstone replace recorded spans", async () => {
  const { host, people, item } = await setup();
  host.spanPageSize = 1;
  const writer = (request: string, text?: string) =>
    host.peopleContext(bob, request, undefined, "Alice", undefined, text === undefined ? undefined : { item, text });
  const { message } = await people.post({ item, text: "Bob speaking" }, writer("same-label", "Bob speaking"));
  const original = host.lastSpan(item);
  assert.equal(host.writeAuthor(item), bob);
  assert.equal((await people.message({ item, message }, host.peopleContext(alice, "read-bob"))).author.id, bob);
  const neighbor = await people.post({ item, text: "Unaffected" }, writer("neighbor", "Unaffected"));
  const before = (await writer("historical-head").call("item_head", { item })).revision;
  await people.edit({ item, message, text: "Bob speaking again" },
    writer("reopen-and-edit", "Bob speaking again"));
  const edited = host.lastSpan(item);
  assert.notEqual(edited, original);
  assert.equal(host.writeAuthor(item), bob);
  const stale = writer("stale-delete");
  const current = (await stale.call("item_head", { item })).revision;
  const historical = await stale.call("item_spans", { item, revision: before });
  assert.equal(historical.spans[0].span, original);
  await assert.rejects(stale.call("item_save", { item, request_id: "stale-span", expected: current,
    replace_span: original, attested: { source: { kind: "empty" } } }), /span unavailable/);
  await people.remove({ item, message }, writer("reopen-and-delete"));
  assert.notEqual(host.lastSpan(item), edited);
  assert.equal(host.writeAuthor(item), undefined);
  assert.equal(decode(host.markdown(item)).messages.find(entry => entry.id === message)?.deleted, true);
  assert.equal((await people.message({ item, message: neighbor.message }, host.peopleContext(alice, "read-neighbor"))).text, "Unaffected");
  assert.doesNotMatch(host.markdown(item), /Bob speaking/);
  assert.deepEqual(host.discoverAsAttempt(alice), []);
  assert.deepEqual(host.discoverAsAttempt(bob), []);
});

test("edit and delete preserve the message link and authorize the original author", async () => {
  const { host, people, item, first, ctx, get } = await setup();
  const target = { item, message: first.message };
  await assert.rejects(people.edit({ ...target, text: "forged" }, ctx(bob, "bad-edit")), /unavailable/i);
  await people.edit({ ...target, text: "Corrected" }, ctx(alice, "edit"));
  assert.equal((await get(bob, first.message)).text, "Corrected");
  assert.equal((await get(bob, first.message)).edited, true);
  await assert.rejects(people.remove(target, ctx(bob, "bad-delete")), /unavailable/i);
  const beforeDeletion = host.files.get(item)!.revision;
  await people.remove(target, ctx(alice, "delete"));
  await assert.rejects(get(bob, first.message), /unavailable/i);
  assert.equal(decode(host.markdown(item)).messages[0].deleted, true);
  assert.equal(decode(host.markdown(item)).messages[0].text, "");
  const source_revision = host.files.get(item)!.revision;
  assert.deepEqual(await handlers["threads.refresh"]({ item, source_revision, anchor: first.message },
    host.context("deleted-anchor", "threads.refresh")), { anchor_unavailable: true });
  assert.deepEqual(await handlers["threads.refresh"]({ item, source_revision: beforeDeletion, anchor: first.message },
    host.context("superseded-deleted-anchor", "threads.refresh")), { anchor_unavailable: true });
  assert.doesNotMatch(JSON.stringify(host.snapshots.get(item)!.view), /Message deleted|Corrected|Alice visible/);
  assert.doesNotMatch(host.markdown(item), /Corrected|Alice visible/);
  assert.ok(host.markdown(item).includes(`{#m-${first.message} `));
});

test("a private read position, draft and mute survive a fresh context for the same actor", async () => {
  const { host, people, item, first, third, ctx } = await setup();
  const sharedHead = host.files.get(item)!.revision;
  await people.setState({ item, readThrough: third.message, draft: "Alice's private draft", muted: true }, ctx(alice, "state"));
  await people.setState({ item, readThrough: first.message, draft: "", muted: false }, ctx(alice, "old-device"));
  assert.deepEqual(await people.getState({ item }, ctx(alice, "fresh")), {
    readThrough: third.message, draft: "", muted: false,
  });
  assert.deepEqual(await people.getState({ item }, ctx(bob, "fresh")), { draft: "", muted: false });
  assert.equal(host.files.get(item)!.revision, sharedHead);
  assert.doesNotMatch(host.markdown(item), /draft|muted|readThrough/);
});

test("an invited agent receives only scoped messages whose authors have consented", async () => {
  const { host, people, item, first, second, third, ctx } = await setup();
  const run = (request: string, scope: { message: string } | { from: string; through: string } | { whole: true }, consent: string[]) =>
    people.callAgent({ item, prompt: "Answer the allowed messages" },
      host.invitedContext(alice, request, { item, scope, consent }));
  await assert.rejects(people.callAgent({ item, prompt: "Try without a grant" }, ctx(alice, "denied")), /invitation/i);
  await assert.rejects(handlers["threads.send"]({ item, text: "Bypass", ...host.choose() },
    host.context("ordinary-send", "threads.send")), /invitation/i);
  await run("single-withheld", { message: second.message }, []);
  await run("caller-old-message-withheld", { message: first.message }, []);
  await run("single", { message: third.message }, [cara]);
  await run("range", { from: first.message, through: second.message }, [alice]);
  await run("whole", { whole: true }, [alice, cara]);
  assert.deepEqual(host.invitedInputs.map(input => input.messages.map(message => message.text)), [
    [], [], ["Cara visible"], ["Alice visible"], ["Alice visible", "Cara visible"],
  ]);
  assert.equal(decode(host.markdown(item)).messages.filter(message => message.author.kind === "agent").length, 5);
});

test("a grant for the previous body cannot disclose an edit or deleted message", async () => {
  const { host, people, item, second, ctx } = await setup();
  const grant = host.invitedContext(alice, "stale", { item, scope: { message: second.message }, consent: [bob] });
  await people.edit({ item, message: second.message, text: "New private text" }, ctx(bob, "edit-private"));
  await people.callAgent({ item, prompt: "Read it" }, grant);
  await people.remove({ item, message: second.message }, ctx(bob, "delete-private"));
  const current = host.invitedContext(alice, "tombstone", { item, scope: { message: second.message }, consent: [bob] });
  await people.callAgent({ item, prompt: "Read it" }, current);
  assert.deepEqual(host.invitedInputs.map(input => input.messages), [[], []]);
});

test("discovery stays per item after a people-thread invitation", async () => {
  const { host, item, people, first } = await setup();
  const { created: ownThread } = await handlers["threads.create"](
    { title: "My agent conversation" }, host.createContext("own-agent", "My agent conversation"),
  );
  assert.deepEqual(host.discoverAsAttempt(alice), [ownThread]);
  assert.deepEqual(host.discoverAsAttempt(bob), []);
  const { job_id } = await handlers["threads.send"]({ item: ownThread, text: "Hello", ...host.choose() },
    host.composerContext("own-agent-send", ownThread, "Hello"));
  await host.run(job_id);
  assert.deepEqual(host.discoverAsAttempt(alice), [ownThread]);

  const invite = host.invitedContext(alice, "explicit-invite", { item, scope: { message: first.message }, consent: [alice] });
  await people.callAgent({ item, prompt: "Read this selection" }, invite);
  assert.deepEqual(host.discoverAsAttempt(alice), [ownThread]);
  assert.deepEqual(host.invitedInputs.map(input => input.messages.map(message => message.text)), [["Alice visible"]]);
});

test("edit accepts the host time just inside 48 hours and expires at the deadline", async () => {
  const { people, item, first, host, get } = await setup();
  const input = { item, message: first.message, text: "Edited in time" };
  await people.edit(input, host.peopleContext(alice, "within", "2026-11-22T11:59:59.999Z"));
  assert.equal((await get(bob, first.message)).edited, true);
  const head = host.files.get(item)!.revision;
  await assert.rejects(people.edit({ ...input, text: "Too late" },
    host.peopleContext(alice, "expired", "2026-11-22T12:00:00Z")), /48 hours/);
  assert.equal(host.files.get(item)!.revision, head);
  assert.equal((await get(bob, first.message)).text, "Edited in time");
});

test("a new reply cannot target a deleted message and an existing reply stays visible", async () => {
  const { host, people, item, first, ctx, get } = await setup();
  const reply = await people.post({ item, text: "Already sent", replyTo: first.message }, ctx(bob, "reply"));
  assert.equal((await get(bob, reply.message)).replyTo, first.message);
  assert.equal((await get(bob, reply.message)).replyQuote, "Alice visible");
  assert.match(host.markdown(item), /> Alice visible\n\nAlready sent/);
  await people.remove({ item, message: first.message }, ctx(alice, "delete-target"));
  await assert.rejects(people.post({ item, text: "Late reply", replyTo: first.message }, ctx(bob, "late-reply")), /unavailable/i);
  assert.equal((await get(bob, reply.message)).text, "Already sent");
  assert.equal((await get(bob, reply.message)).replyTo, undefined);
  assert.equal((await get(bob, reply.message)).replyQuote, undefined);
  assert.doesNotMatch(host.markdown(item), /Alice visible/);
  assert.equal(decode(host.markdown(item)).messages.find(entry => entry.id === reply.message)?.replyTo, first.message);
});

test("checked quotes retain the original author, survive a source edit and are redacted with it", async () => {
  const { host, people, item, ctx } = await setup();
  const writer = (person: string, request: string, text: string) =>
    host.peopleContext(person, request, undefined, undefined, undefined, { item, text });
  const original = "First paragraph\n\nSecond paragraph";
  const { message: target } = await people.post({ item, text: original }, writer(alice, "two-paragraphs", original));
  const text = "> My own words\n\nThe reply";
  const replyWriter = writer(bob, "quoted-reply", text);
  const { message } = await people.post({ item, text, replyTo: target }, replyWriter);
  assert.match(host.markdown(item), /> First paragraph\n> \n> Second paragraph\n\n> My own words/);
  assert.equal((await people.message({ item, message }, ctx(alice, "read-quote"))).replyQuote, original);
  const { revision } = await replyWriter.call("item_head", { item });
  const { spans } = await replyWriter.call("item_spans", { item, revision });
  const span = spans.at(-1)!;
  assert.equal(Buffer.from(host.markdown(item)).subarray(span.middle_offset, span.middle_offset + span.middle_length).toString(), text);
  assert.deepEqual(span.attribution, { kind: "person", actor: bob });
  assert.deepEqual(span.quote?.attribution, { kind: "person", actor: alice });
  const { message: nested } = await people.post({ item, text: "Acknowledged", replyTo: message },
    writer(alice, "nested-quote", "Acknowledged"));
  assert.equal((await people.message({ item, message: nested }, ctx(bob, "read-nested-quote"))).replyQuote, text);

  await people.edit({ item, message: target, text: "Changed" }, writer(alice, "edit-quoted-source", "Changed"));
  assert.equal((await people.message({ item, message }, ctx(alice, "read-original-quote"))).replyQuote, original);
  await people.remove({ item, message: target }, ctx(alice, "delete-quoted-source"));
  assert.doesNotMatch(host.markdown(item), /First paragraph|Second paragraph|Changed/);
  const live = await replyWriter.call("item_head", { item });
  const after = (await replyWriter.call("item_spans", { item, revision: live.revision })).spans.find(entry => entry.span === span.span);
  assert.ok(after);
  assert.equal(after.quote, undefined);
  assert.deepEqual(after.attribution, span.attribution);
  assert.equal((await people.message({ item, message }, ctx(alice, "read-after-redaction"))).text, text);
  await assert.rejects(replyWriter.call("item_range", { item, revision, offset: 0, length: 10 }), /withheld after redaction/);

  const longQuote = "Long quote ".repeat(100);
  const { message: longTarget } = await people.post({ item, text: longQuote }, writer(alice, "long-quote-target", longQuote));
  const { message: longReply } = await people.post({ item, text, replyTo: longTarget }, writer(bob, "long-quoted-reply", text));
  assert.equal((await people.message({ item, message: longReply }, ctx(alice, "read-long-quote"))).replyQuote, longQuote);
  assert.equal(host.writeAuthor(item), bob);
  const longHead = await replyWriter.call("item_head", { item });
  const longSpan = (await replyWriter.call("item_spans", { item, revision: longHead.revision })).spans.at(-1)!;
  assert.ok(longSpan.quote && longSpan.quote.length > 512);
  await people.edit({ item, message: longTarget, text: "Revised source" }, writer(alice, "change-long-source", "Revised source"));
  await people.edit({ item, message: longReply, text: "Revised reply" }, writer(bob, "change-long-reply", "Revised reply"));
  const revised = await people.message({ item, message: longReply }, ctx(alice, "read-revised-reply"));
  assert.equal(revised.replyQuote, longQuote);
  assert.equal(host.writeAuthor(item), undefined);
});

test("muted row counts are private and count a mention plus reply on one message once", async () => {
  const { people, item, first, third, ctx } = await setup();
  await people.setState({ item, readThrough: third.message, draft: "", muted: true }, ctx(alice, "mute"));
  const addressed = await people.post({ item, text: "For Alice", mentions: [alice], replyTo: first.message }, ctx(bob, "mention-and-reply"));
  const deleted = await people.post({ item, text: "Will disappear", mentions: [alice] }, ctx(bob, "removed-mention"));
  await people.post({ item, text: "For Cara", mentions: [cara] }, ctx(bob, "mention-cara"));
  await people.remove({ item, message: deleted.message }, ctx(bob, "delete-mention"));
  assert.deepEqual(await people.row({ item }, ctx(alice, "row-a")), { unread: 2, mentions: 1, replies: 1, muted: true, attention: 1 });
  assert.deepEqual(await people.row({ item }, ctx(cara, "row-c")), { unread: 4, mentions: 1, replies: 0, muted: false, attention: 1 });
  await people.setPreferences({ receiptsAndTyping: true, push: { enabled: true, iosLocked: true, iosUnlocked: true } },
    ctx(alice, "muted-push-enabled"));
  assert.deepEqual(await people.delivery({ item, message: addressed.message, device: "ios", locked: true }, ctx(alice, "muted-delivery")),
    { ring: false, push: false });
  await people.setState({ item, readThrough: addressed.message, draft: "", muted: true }, ctx(alice, "read"));
  assert.deepEqual(await people.row({ item }, ctx(alice, "row-after-read")), { unread: 1, mentions: 0, replies: 0, muted: true, attention: 0 });
});

test("private preferences combine receipts and typing and apply iOS push state", async () => {
  const { people, item, second, ctx } = await setup();
  await people.setPreferences({ receiptsAndTyping: false,
    push: { enabled: true, iosLocked: true, iosUnlocked: false } }, ctx(alice, "preferences"));
  assert.match((await ctx(alice, "read-preference-file").privatePreferences?.read())!,
    /^read-receipts-and-typing: false\npush-notifications: true\npush-when-ios-locked: true\npush-when-ios-unlocked: false$/m);
  assert.equal((await people.getPreferences(ctx(bob, "bob-prefs"))).receiptsAndTyping, true);
  assert.equal((await people.getPreferences(ctx(alice, "alice-prefs"))).receiptsAndTyping, false);
  const decision = (locked: boolean) => people.delivery({ item, message: second.message, device: "ios", locked }, ctx(alice, "delivery"));
  assert.deepEqual(await decision(true), { ring: true, push: true });
  assert.deepEqual(await decision(false), { ring: true, push: false });
});

test("the conversation file example has two people, one agent and a separate private state record", async () => {
  const host = new FakeHost();
  const people = peopleFromBundle();
  const context = (actor: string, request: string, at: string) => host.peopleContext(actor, request, `2026-11-20T${at}:00Z`);
  const { created: item } = await people.create({ title: "Community Garden", participants: [bob] }, context(alice, "example-create", "09:00"));
  const write = (actor: string, request: string, at: string, text?: string) =>
    host.peopleContext(actor, request, `2026-11-20T${at}:00Z`, undefined, undefined, text === undefined ? undefined : { item, text });
  const first = "Can we meet by the garden gate at 10?";
  const answer = await people.post({ item, text: first }, write(alice, "example-first", "09:01", first));
  const reply = "Yes, I’ll bring the key.";
  await people.post({ item, text: reply, replyTo: answer.message, mentions: [alice] },
    write(bob, "example-reply", "09:02", reply));
  const mistaken = "I was looking at the wrong date.";
  const discarded = await people.post({ item, text: mistaken }, write(bob, "example-deleted", "09:03", mistaken));
  const changed = "Can we meet by the garden gate at 10:30?\n\nThe rain should clear by then.";
  await people.edit({ item, message: answer.message, text: changed },
    write(alice, "example-edit", "09:04", changed));
  await people.remove({ item, message: discarded.message },
    write(bob, "example-delete", "09:05"));
  const invite = host.invitedContext(alice, "example-agent", { item, scope: { message: answer.message }, consent: [alice] },
    "2026-11-20T09:06:00Z");
  await people.callAgent({ item, prompt: "Summarize our meeting time." }, invite);
  await people.consult({ item, prompt: "Consult another agent on a wet-weather backup." },
    host.consultationContext(alice, "example-consultation", "2026-11-20T09:07:00Z"));
  await people.setState({ item, readThrough: answer.message, draft: "I’ll bring the gloves.", muted: true },
    context(alice, "example-state", "09:07"));

  const thread = decode(host.markdown(item));
  assert.deepEqual(thread.participants.map(participant => participant.kind), ["person", "person", "agent"]);
  assert.equal(thread.messages[0].edited, true);
  assert.equal(thread.messages[2].deleted, true);
  assert.ok(thread.messages[4].consultation);
  assert.match(host.markdown(item), /^format: calternal-thread\/v2$/m);
  assert.match(host.markdown(item), /^agent-p3-plugin: calternal-threads-claude-code\nagent-p3-harness: claude-code$/m);
  assert.match((await context(alice, "example-read-state-file", "09:07").privateState?.read(item))!,
    /^last-read-message: "b31395e111a687c4"$/m);
  assert.doesNotMatch(host.markdown(item), /gloves|wrong date/);
  const directory = process.env.THREADS_REPORT_FILES;
  if (directory) await host.materialize(directory, item, alice);
});

test("a displayed name and time survive a hand edit of the body without a length field", async () => {
  const { host, item, first, second, people, ctx } = await setup();
  const old = host.markdown(item);
  assert.match(old, /### Alice · 2026-11-20 12:00 UTC \{#m-[a-f0-9]{16} /);
  assert.match(old, /participants:\n  - p1 \| Alice \| person/);
  assert.doesNotMatch(old, /<!--|length|id="message/);
  const updated = old.replace("Alice visible", "The longer hand-edited body.\n\n\\### Someone · 2026-11-20 12:00 UTC {#m-aabbccddeeff0011 by=p2}");
  const { publish } = await host.stageUnproven(item, Buffer.from(updated));
  await publish();
  assert.equal((await people.message({ item, message: first.message }, ctx(bob, "read-edited-file"))).text,
    "The longer hand-edited body.\n\n### Someone · 2026-11-20 12:00 UTC {#m-aabbccddeeff0011 by=p2}");
  assert.equal((await people.message({ item, message: second.message }, ctx(alice, "read-next"))).text, "Bob private");
});

test("an audience-checked rename appears in new headings and keeps the old name and stable actor ID", async () => {
  const { host, item, people, first, get } = await setup();
  const newer = await people.post({ item, text: "After a name change" },
    host.peopleContext(alice, "rename-new-message", "2026-11-20T12:07:12.345Z", "Alice · AJ | New"));
  const old = await get(bob, first.message);
  const changed = await get(bob, newer.message);
  assert.equal(old.author.name, "Alice");
  assert.equal(changed.author.name, "Alice · AJ | New");
  assert.equal(changed.author.id, old.author.id);
  assert.equal(changed.posted_at, "2026-11-20T12:07:12.345Z");
  assert.match(host.markdown(item), /  - p1 \| Alice \| person/);
  assert.match(host.markdown(item), /### Alice \\· AJ \\\| New · 2026-11-20 12:07:12\.345 UTC/);
});

test("one starter-owned file gives both people full access", async () => {
  const host = new FakeHost();
  const people = peopleFromBundle();
  const { created: item } = await people.create({ title: "Bob started", participants: [alice] }, host.peopleContext(bob, "bob-start"));
  assert.equal(host.files.size, 1);
  assert.equal(host.files.get(item)?.space, host.personalSpace(bob));
  assert.equal(host.conversationAccess(item, alice), "full");
  assert.equal(host.conversationAccess(item, cara), undefined);
  const sent = await people.post({ item, text: "Same file" }, host.peopleContext(alice, "shared-write"));
  assert.equal((await people.message({ item, message: sent.message }, host.peopleContext(bob, "shared-read"))).text, "Same file");
});

test("the people picker and new shares use current shared-group visibility", async () => {
  const host = new FakeHost();
  const people = peopleFromBundle();
  const pendingInvite = "019a0000-0000-7000-8000-000000000004";
  host.setDirectoryGroups([
    { id: "another-group", source: "local", members: [alice, cara] },
    { id: "checked-group", source: "oidc", members: [alice, bob] },
  ]);
  const ctx = host.peopleContext(alice, "pick", undefined, undefined, "checked-group");
  assert.deepEqual((await ctx.visiblePeople!()).map(({ id }) => id), [alice, bob]);
  assert.deepEqual((await ctx.visiblePeople!([cara, pendingInvite])), []);
  for (const id of [cara, pendingInvite]) {
    await assert.rejects(people.create({ title: "Unavailable", participants: [id] }, ctx), /^Error: Participants unavailable\.$/);
  }
  assert.equal(host.files.size, 0);
  const other = host.peopleContext(alice, "other-picker", undefined, undefined, "another-group");
  assert.deepEqual((await other.visiblePeople!()).map(({ id }) => id), [alice, cara]);
  await assert.rejects(people.create({ title: "Mixed", participants: [bob, cara] }, ctx), /Participants unavailable/);

  const input = { title: "Named share", participants: [bob] };
  const { created: item } = await people.create(input, ctx);
  assert.deepEqual(decode(host.markdown(item)).participants.map(({ id }) => id), [alice, bob]);
  host.setDirectoryGroups([]);
  assert.deepEqual(await ctx.visiblePeople!(), []);
  await assert.rejects(people.create(input, ctx), /Participants unavailable/);
  assert.equal(host.conversationAccess(item, bob), "read");
});

test("losing the last common group preserves an exportable transcript and blocks both participants from writing", async () => {
  const host = new FakeHost();
  const people = peopleFromBundle();
  const ctx = (actor: string, request: string) => host.peopleContext(actor, request);
  const { created: item } = await people.create({ title: "Shared history", participants: [bob] }, ctx(alice, "start"));
  const first = await people.post({ item, text: "Keep this" }, ctx(alice, "before"));
  const invitations = [alice, bob].map(actor => host.invitedContext(actor, `invite-${actor}`, {
    item, scope: { message: first.message }, consent: [alice],
  }));
  const original = host.markdown(item);
  host.setDirectoryGroups([
    { id: "left", source: "local", members: [alice] },
    { id: "right", source: "oidc", members: [bob] },
  ]);

  for (const [index, actor] of [alice, bob].entries()) {
    const read = ctx(actor, "read");
    assert.equal((await people.message({ item, message: first.message }, read)).text, "Keep this");
    const { revision } = await read.call("item_head", { item });
    const { bytes } = await read.call("item_range", { item, revision, offset: 0, length: 65536 });
    assert.equal(Buffer.from(bytes, "base64").toString(), original);
    await assert.rejects(people.post({ item, text: "Blocked" }, ctx(actor, "post")), /read-only/);
    await assert.rejects(people.callAgent({ item, prompt: "Blocked" }, invitations[index]), /read-only/);
    await assert.rejects(people.consult({ item, prompt: "Blocked" },
      host.consultationContext(actor, "consult")), /read-only/);
  }
  await assert.rejects(people.edit({ item, message: first.message, text: "Changed" }, ctx(alice, "edit")), /read-only/);
  await assert.rejects(people.remove({ item, message: first.message }, ctx(alice, "delete")), /read-only/);
  await assert.rejects(host.rename(item, "Changed", "rename", alice), /read-only/);
  assert.deepEqual(host.invitedInputs, []);
  assert.deepEqual(host.consultationTurns, []);
  assert.equal(host.markdown(item), original);
});

test("losing a common group during an agent call fences tool reads and the pending reply", async () => {
  const { host, people, item, first } = await setup();
  const invitation = host.invitedContext(alice, "active-invitation", {
    item, scope: { message: first.message }, consent: [alice],
  });
  const original = host.markdown(item);
  host.onInvited = async ({ readMessage }) => {
    assert.equal(readMessage(first.message)?.text, "Alice visible");
    host.setDirectoryGroups([]);
    assert.equal(readMessage(first.message), undefined);
  };
  await assert.rejects(people.callAgent({ item, prompt: "Summarize" }, invitation), /read-only|consent changed/);
  assert.equal(host.markdown(item), original);
});

test("a membership change between picker lookup and shared creation blocks the write", async () => {
  const host = new FakeHost();
  const people = peopleFromBundle();
  const ctx = host.peopleContext(alice, "stale-picker");
  await assert.rejects(people.create({ title: "Uncommitted", participants: [bob] }, {
    ...ctx,
    createShared: async (input, ids) => {
      host.setDirectoryGroups([]);
      return ctx.createShared!(input, ids);
    },
  }), /Participants unavailable/);
  assert.equal(host.files.size, 0);
});

test("a call keeps its selection across later posts, with revocable one-call and always consent", async () => {
  const { host, people, ctx, item, first, second, post } = await setup();
  host.setAlwaysConsent(item, bob, true);
  const call = (request: string, consent: string[]) => host.invitedContext(alice, request, { item, scope: { whole: true }, consent: [alice, ...consent] });
  const original = call("snapshot", [cara]);
  let savedTool: ((id: string) => { text: string } | undefined) | undefined;
  const late = await post(bob, "after-call", "Too late for the first selection");
  host.onInvited = async ({ readMessage }) => {
    assert.equal(readMessage(second.message)?.text, "Bob private");
    assert.equal(readMessage(late.message), undefined);
    savedTool = readMessage;
  };
  await people.callAgent({ item, prompt: "Answer" }, original);
  assert.equal(savedTool?.(second.message), undefined);
  host.onInvited = async () => {};
  assert.deepEqual(host.invitedInputs.at(-1)?.messages.map(message => message.text), ["Alice visible", "Bob private", "Cara visible"]);

  await people.callAgent({ item, prompt: "Second call" }, call("later", []));
  assert.deepEqual(host.invitedInputs.at(-1)?.messages.map(message => message.text), ["Alice visible", "Bob private", "Too late for the first selection"]);
  host.onInvited = async ({ readMessage }) => {
    host.setAlwaysConsent(item, bob, false);
    assert.equal(readMessage(second.message), undefined);
  };
  await assert.rejects(people.callAgent({ item, prompt: "Revoke during call" }, call("revoke-active", [])), /consent changed/);
  host.onInvited = async () => {};
  await people.callAgent({ item, prompt: "Third call" }, call("revoked", []));
  assert.deepEqual(host.invitedInputs.at(-1)?.messages.map(message => message.id), [first.message]);
  assert.equal((await people.message({ item, message: late.message }, ctx(bob, "person-read"))).text, "Too late for the first selection");
});

test("a consultation can go beyond three exchanges and posts one result when the agents finish", async () => {
  const { host, people, item } = await setup();
  const unstamped = host.peopleContext(alice, "unchecked-consultation");
  await assert.rejects(people.consult({ item, prompt: "Ask for a recommendation" }, unstamped), /checked consultation/i);
  host.consultationExchanges = 5;
  const ctx = host.consultationContext(alice, "consultation");
  const output = await people.consult({ item, prompt: "Ask for a recommendation" }, ctx);
  assert.equal(host.consultationTurns.length, 11);
  assert.equal(host.consultationTurns.at(-1)?.role, "summary");
  const thread = decode(host.markdown(item));
  assert.equal(thread.messages.length, 4);
  assert.equal(thread.messages[3].id, output.message);
  assert.equal(thread.messages[3].text, "Agreed recommendation");
  assert.equal(thread.messages[3].consultation?.id, output.consultation.id);
  assert.doesNotMatch(host.markdown(item), /Private exchange/);
  assert.match(host.markdown(output.consultation.id), /^## Consulted agent\n\nPrivate exchange 2$/m);
});

test("Stop ends a consultation without posting to the conversation", async () => {
  const { host, people, item } = await setup();
  const stop = new AbortController();
  host.onConsultationTurn = async () => stop.abort(new Error("Consultation stopped"));
  const before = host.markdown(item);
  await assert.rejects(people.consult({ item, prompt: "Find a recommendation" },
    host.consultationContext(alice, "consult-stop", undefined, stop.signal)), /Consultation stopped/);
  assert.equal(host.markdown(item), before);
  const log = [...host.files].find(([, file]) => file.title === "Agent consultation");
  assert.ok(log);
  assert.match(host.markdown(log[0]), /Private exchange 1/);
  assert.doesNotMatch(host.markdown(log[0]), /## Summary/);
});

test("missing consent waits one minute and includes only authors who reply in time", async () => {
  const { host, people, item } = await setup();
  host.setAlwaysConsent(item, alice, true);
  host.onConsentWait = async () => [{ author: bob, after: 59 }, { author: cara, after: 61 }];
  await people.callAgent({ item, prompt: "Answer after waiting" },
    host.invitedContext(alice, "consent-deadline", { item, scope: { whole: true }, consent: [] }));
  assert.deepEqual(host.consentWaits, [{ authors: [bob, cara], seconds: 60 }]);
  assert.deepEqual(host.invitedInputs[0].messages.map(message => message.text), ["Alice visible", "Bob private"]);
});

test("ask-each-time authorization expires and is never reused by a later call", async () => {
  const { host, people, item } = await setup();
  host.setAlwaysConsent(item, alice, true);
  const call = (name: string, consent: string[]) =>
    host.invitedContext(alice, name, { item, scope: { whole: true }, consent });
  const approved = call("one-approved-call", [bob]);
  await people.callAgent({ item, prompt: "Summarize" }, approved);
  await people.callAgent({ item, prompt: "Summarize" }, approved);
  assert.equal(host.invitedInputs.length, 1);
  assert.deepEqual(host.invitedInputs[0].messages.map(message => message.text), ["Alice visible", "Bob private"]);
  await people.callAgent({ item, prompt: "Summarize again" }, call("no-later-approval", []));
  host.invitationClock = (at: string) => Date.parse(at) + 5 * 60_000;
  await people.callAgent({ item, prompt: "Summarize after expiry" }, call("expired-approval", [bob]));
  assert.deepEqual(host.invitedInputs.slice(1).map(input => input.messages.map(message => message.text)),
    [["Alice visible"], ["Alice visible"]]);
});
