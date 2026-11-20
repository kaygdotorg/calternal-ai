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
  await people.remove(target, ctx(alice, "delete"));
  const deleted = await get(bob, first.message);
  assert.equal(deleted.id, first.message);
  assert.equal(deleted.deleted, true);
  assert.equal(deleted.text, undefined);
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
