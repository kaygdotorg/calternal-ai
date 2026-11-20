import assert from "node:assert/strict";
import test, { after } from "node:test";
import type { ItemSnapshotView } from "../generated/wire.js";
import { checkTrace, FakeHost, handlers, peopleFromBundle } from "./host.ts";

after(checkTrace);

const alice = "019a0000-0000-7000-8000-000000000001";
const bob = "019a0000-0000-7000-8000-000000000002";

function files(view?: ItemSnapshotView) {
  return view?.content.type === "stack" ? view.content.children.filter(node => node.type === "snapshot-attachment") : [];
}

test("a shared message commits checked files once and reopens them within the viewer's scope", async () => {
  const host = new FakeHost();
  const people = peopleFromBundle();
  const { created: item } = await people.create({ title: "Files", participants: [bob] }, host.peopleContext(alice, "new-files"));
  const { created: other } = await people.create({ title: "Another conversation", participants: [bob] }, host.peopleContext(alice, "other-files"));
  const choice = host.upload(item, bob, "Meeting [notes] 🌏.pdf", "private bytes");
  const second = host.upload(item, bob, "Agenda.txt", "another file");
  const text = "Here are the notes.";
  const writer = (actor: string, request: string, target = item) =>
    host.peopleContext(actor, request, undefined, undefined, undefined, { item: target, text });
  const post = { item, text, attachments: [choice, second] };
  const prior = host.files.get(item)!.revision;
  await assert.rejects(people.post(post, writer(alice, "wrong-uploader")), /Attachment unavailable/);
  await assert.rejects(people.post({ ...post, item: other }, writer(bob, "wrong-item", other)), /Attachment unavailable/);
  await assert.rejects(people.post({ ...post, attachments: [choice, choice] }, writer(bob, "duplicate")), /four distinct files/);
  assert.equal(host.files.get(item)!.revision, prior);
  assert.deepEqual(files(host.snapshots.get(item)?.view), []);

  host.loseNextAppendAck = true;
  await assert.rejects(people.post(post, writer(bob, "post-file")), /acknowledgement lost/);
  const { message } = await people.post(post, writer(bob, "post-file"));
  await assert.rejects(people.post(post, writer(bob, "reuse-file")), /Attachment unavailable/);
  assert.equal(host.thread(item).messages.length, 1);
  assert.match(host.markdown(item), /\[Meeting \\\[notes\\\] 🌏\.pdf\]\(<Attachments\/[a-f0-9]{16}\/Meeting%20%5Bnotes%5D%20%F0%9F%8C%8F\.pdf>\)/);
  assert.doesNotMatch(host.markdown(item), /upload-|private bytes/);
  assert.equal(files(host.snapshots.get(item)?.view)[0]?.label, "Meeting [notes] 🌏.pdf");

  const live = () => handlers["chat.refresh"]({ item, source_revision: host.files.get(item)!.revision, anchor: message },
    host.peopleContext(alice, "open-file"));
  const [asset, another] = files((await live()).view);
  assert.equal(host.attachmentRange(asset.ref, alice, 0, 7).toString(), "private");
  assert.throws(() => host.attachmentRange(asset.ref, bob, 0, 7), /Attachment unavailable/);
  await people.edit({ item, message, text: "Edited text" },
    host.peopleContext(bob, "edit-file-message", undefined, undefined, undefined, { item, text: "Edited text" }));
  assert.equal(files((await live()).view)[0]?.label, asset.label);
  await people.post({ item, text: "An ordinary later message" }, host.peopleContext(alice, "later-opaque"));
  assert.equal(files((await live()).view)[0]?.label, asset.label);
  host.revokeAttachment(choice);
  assert.throws(() => host.attachmentRange(asset.ref, alice, 0, 7), /Attachment unavailable/);
  assert.deepEqual(files((await live()).view).map(file => file.label), ["Agenda.txt"]);
  assert.doesNotMatch(JSON.stringify(await people.message({ item, message }, writer(alice, "direct-message"))), /Meeting|private bytes/);
  await people.remove({ item, message }, host.peopleContext(bob, "delete-with-file"));
  assert.doesNotMatch(host.markdown(item), /Meeting|Attachments\//);
  assert.match(host.markdown(item), /deleted/);
  assert.throws(() => host.attachmentRange(another.ref, alice, 0, 7), /Attachment unavailable/);
});

test("Send pins a staged upload to one job and records it on the authored prompt", async () => {
  const host = new FakeHost();
  const text = "Review this file.";
  const { created: item } = await handlers["chat.create"]({ title: "File send" }, host.createContext("new", "File send"));
  const choice = host.upload(item, alice, "agenda.txt", "attachment contents");
  const longName = host.upload(item, alice, "🌏".repeat(40) + ".txt", "attachment contents");
  await assert.rejects(handlers["chat.send"]({ item, text, ...host.choose(), attachments: [longName] },
    host.composerContext("over-budget", item, text)), /attachment names are too long/);
  assert.equal(host.jobs.size, 0);
  const input = { item, text, ...host.choose(), attachments: [choice] };
  const send = (request: string) => handlers["chat.send"](input, host.composerContext(request, item, text));
  const cancelled = await send("cancel-upload");
  host.stop(item, { intent: cancelled.job_id });
  host.settle(cancelled.job_id);
  const { job_id } = await send("send-file");
  assert.deepEqual(await send("send-file"), { job_id });
  await assert.rejects(send("another-send"), /Attachment unavailable/);
  assert.deepEqual(host.jobs.get(job_id)?.context.contribution_input, { text });
  assert.doesNotMatch(host.markdown(item), /agenda\.txt/);
  await host.run(job_id);
  await host.replay(job_id);
  const [prompt, reply] = host.thread(item).messages;
  assert.equal(prompt.files?.[0].name, "agenda.txt");
  assert.equal(reply.files, undefined);
  assert.equal(host.files.get(item)?.discovery, "creator_only");
  assert.equal(files(host.snapshots.get(item)?.view).length, 1);
});
