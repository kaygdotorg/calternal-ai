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
  assert.deepEqual(decode(host.markdown(item)).participants, [alice, bob, cara].map(id => ({ kind: "person", id })));
  assert.notEqual(first.message, second.message);
  assert.deepEqual(await post(alice, "a1", "Alice visible"), first);
  assert.equal((await get(bob, first.message)).text, "Alice visible");
  assert.match(host.markdown(item), new RegExp(`id="message-${first.message}"`));
  await assert.rejects(get("outsider", first.message), /unavailable/i);
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
  assert.match(host.markdown(item), new RegExp(`id="message-${first.message}"`));
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
  await assert.rejects(run("single-withheld", { message: second.message }, []), /no readable messages/i);
  await run("single", { message: third.message }, [third.message]);
  await run("range", { from: first.message, through: second.message }, []);
  await run("whole", { whole: true }, [third.message]);
  assert.deepEqual(host.invitedInputs.map(input => input.messages.map(message => message.text)), [
    ["Cara visible"], ["Alice visible"], ["Alice visible", "Cara visible"],
  ]);
  assert.equal(decode(host.markdown(item)).messages.filter(message => message.author.kind === "agent").length, 3);
});

test("a grant for the previous body cannot disclose an edit or deleted message", async () => {
  const { host, people, item, second, ctx } = await setup();
  const grant = host.invitedContext(alice, "stale", { item, scope: { message: second.message }, consent: [second.message] });
  await people.edit({ item, message: second.message, text: "New private text" }, ctx(bob, "edit-private"));
  await assert.rejects(people.callAgent({ item, prompt: "Read it" }, grant), /invitation/i);
  await people.remove({ item, message: second.message }, ctx(bob, "delete-private"));
  const current = host.invitedContext(alice, "tombstone", { item, scope: { message: second.message }, consent: [second.message] });
  await assert.rejects(people.callAgent({ item, prompt: "Read it" }, current), /no readable messages/i);
  assert.deepEqual(host.invitedInputs, []);
});

test("discovery uses the checked creator policy per item and excludes a people thread", async () => {
  const { host, item, people, first } = await setup();
  const { created: ownThread } = await handlers["threads.create"](
    { title: "My agent conversation" }, host.context("own-agent", "threads.create"),
  );
  const { job_id } = await handlers["threads.send"]({ item: ownThread, text: "Hello", ...host.choose() },
    host.context("own-agent-send", "threads.send"));
  await host.run(job_id);
  assert.deepEqual(host.discoverAsAttempt(alice), [ownThread]);
  assert.deepEqual(host.discoverAsAttempt(bob), []);

  const invite = host.invitedContext(alice, "explicit-invite", { item, scope: { message: first.message }, consent: [] });
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
  await people.remove({ item, message: first.message }, ctx(alice, "delete-target"));
  await assert.rejects(people.post({ item, text: "Late reply", replyTo: first.message }, ctx(bob, "late-reply")), /unavailable/i);
  assert.equal((await get(bob, reply.message)).text, "Already sent");
  assert.equal((await get(bob, reply.message)).replyTo, undefined);
  assert.equal(decode(host.markdown(item)).messages.find(entry => entry.id === reply.message)?.replyTo, first.message);
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
  const answer = await people.post({ item, text: "Can we meet by the garden gate at 10?" }, context(alice, "example-first", "09:01"));
  await people.post({ item, text: "Yes, I’ll bring the key.", replyTo: answer.message, mentions: [alice] },
    context(bob, "example-reply", "09:02"));
  const discarded = await people.post({ item, text: "I was looking at the wrong date." }, context(bob, "example-deleted", "09:03"));
  await people.edit({ item, message: answer.message, text: "Can we meet by the garden gate at 10:30?" },
    context(alice, "example-edit", "09:04"));
  await people.remove({ item, message: discarded.message }, context(bob, "example-delete", "09:05"));
  const invite = host.invitedContext(alice, "example-agent", { item, scope: { message: answer.message }, consent: [] },
    "2026-11-20T09:06:00Z");
  await people.callAgent({ item, prompt: "Summarize our meeting time." }, invite);
  await people.setState({ item, readThrough: answer.message, draft: "I’ll bring the gloves.", muted: true },
    context(alice, "example-state", "09:07"));

  const thread = decode(host.markdown(item));
  assert.deepEqual(thread.participants.map(participant => participant.kind), ["person", "person", "agent"]);
  assert.equal(thread.messages[0].edited, true);
  assert.equal(thread.messages[2].deleted, true);
  assert.doesNotMatch(host.markdown(item), /gloves|wrong date/);
  const directory = process.env.THREADS_REPORT_FILES;
  if (directory) await host.materialize(directory, item, alice);
});
