import type { SnapshotNode } from "../generated/wire.js";
import assert from "node:assert/strict";
import test, { after } from "node:test";
import { decode, step } from "../src/format.ts";
import { FakeHost, handlers, checkTrace } from "./host.ts";

after(checkTrace);

async function setup() {
  const host = new FakeHost();
  const context = (request: string, operation: string) => host.context(request, operation);
  const { created: item } = await handlers["threads.create"]({ title: "Review the lease" }, context("create-1", "threads.create"));
  const selection = host.choose();
  const send = (request: string, text: string) =>
    handlers["threads.send"]({ item, text, ...selection }, context(request, "threads.send"));
  return { host, item, send, selection, context };
}

test("create, select a contributed harness, append its reply and Stop an exact attempt", async () => {
  const { host, item, send, context } = await setup();
  assert.match(host.files.get(item)!.path!, /^Threads\/2026-11-20 Review the lease\/Conversation\.md$/);
  assert.deepEqual(decode(host.markdown(item)).participants.map(p => p.kind), ["person"]);
  const first = await send("send-1", "Say hello 🦊\n\n---\n<!-- ## Agent -->");
  assert.deepEqual(await send("send-1", "Say hello 🦊\n\n---\n<!-- ## Agent -->"), first);
  await host.run(first.job_id);
  const thread = decode(host.markdown(item));
  const reply = thread.messages[1];
  assert.deepEqual(thread.participants.map(p => p.kind), ["person", "agent"]);
  assert.equal(reply.text, "Reply to Say hello 🦊\n\n---\n<!-- ## Agent -->");
  assert.match(host.markdown(reply.log!.id), /## Reply/);
  assert.ok(host.markdown(item).includes(`{#m-${reply.id} `));
  assert.equal((await handlers["threads.message"]({ item, message: reply.id }, context("read-1", "threads.message"))).text, reply.text);
  assert.equal(host.snapshots.get(item)?.source_revision, host.files.get(item)?.revision);
  assert.match(JSON.stringify(host.snapshots.get(item)!.view), /Reply to Say hello/);

  let reached: () => void;
  const started = new Promise<void>(resolve => { reached = resolve; });
  let release: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  host.claude = async () => { reached(); await gate; return "late reply"; };
  const second = await send("send-2", "Keep working");
  const run = host.run(second.job_id);
  await started;
  assert.throws(() => host.stop(item, { execution: host.jobs.get(first.job_id)!.attempt!.execution }), /stale Stop/);
  host.stop(item, { execution: host.jobs.get(second.job_id)!.attempt!.execution });
  release!();
  await assert.rejects(run, /attempt retired/);
  assert.doesNotMatch(host.markdown(item), /late reply/);
  const logs = [...host.files].filter(([, file]) => file.owner === "claude-code-run");
  assert.equal(logs.length, 2);
  assert.match(host.markdown(logs[1][0]), /## Prompt\n\nKeep working/);
  assert.doesNotMatch(host.markdown(logs[1][0]), /## Reply/);
});

test("one durable successor waits for the head and exact attempt teardown", async () => {
  const { host, item, send } = await setup();
  const first = await send("queue-head", "First");
  const queued = await send("queue-next", "Second");
  assert.deepEqual(await send("queue-next", "Second"), queued);
  await assert.rejects(send("queue-full", "Third"), /pending_limit/);
  await assert.rejects(host.run(queued.job_id), /successor must wait/);
  let reached: () => void;
  const started = new Promise<void>(resolve => { reached = resolve; });
  let release: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  host.claude = async () => { reached(); await gate; return "retired"; };
  const run = host.run(first.job_id);
  await started;
  host.stop(item, { execution: host.jobs.get(first.job_id)!.attempt!.execution });
  await assert.rejects(host.run(queued.job_id), /successor must wait for teardown/);
  release!();
  await assert.rejects(run, /attempt retired/);
  host.settle(first.job_id);
  host.claude = async text => `Reply to ${text}`;
  await host.run(queued.job_id);
  assert.deepEqual(decode(host.markdown(item)).messages.map(message => message.text), ["First", "Second", "Reply to Second"]);
});

test("rename preserves IDs and old read pins, archive fences pending work", async () => {
  const { host, item, send, context } = await setup();
  const original = host.snapshots.get(item)!;
  const { job_id } = await send("rename-head", "First");
  await host.run(job_id);
  const before = decode(host.markdown(item));
  const path = host.files.get(item)!.path;
  await handlers["threads.rename"]({ item, title: "Later title" }, context("rename-1", "threads.rename"));
  const after = decode(host.markdown(item));
  assert.equal(after.title, "Later title");
  assert.equal(after.id, before.id);
  assert.deepEqual(after.messages, before.messages);
  assert.equal(host.files.get(item)!.path, path);
  assert.deepEqual(await handlers["threads.refresh"]({
    item, source_revision: original.source_revision,
  }, context("refresh-1", "threads.refresh")), { view: original.view });
  assert.equal(host.snapshots.get(item)!.source_revision, host.files.get(item)!.revision);

  const pending = await send("pre-archive", "Never start");
  const queued = await send("pre-archive-queued", "Never promote");
  host.archive(item);
  assert.equal(host.jobs.get(queued.job_id)!.state, "stopped");
  assert.equal(host.jobs.get(pending.job_id)!.state, "stopped");
  await assert.rejects(send("post-archive", "Never admit"), /target unavailable/);
  assert.equal(decode(host.markdown(item)).messages.length, 2);
});

test("alarm delivery retains checked harness and lets the worker decide whether to run late", async () => {
  const { host, item, selection, context } = await setup();
  const arm = (if_missed: "skip" | "run", request: string) =>
    handlers["threads.arm"]({
      item, harness: selection.harness, text: if_missed, at: "2026-11-20T13:00:00Z", if_missed, grace_seconds: 30,
    }, context(request, "threads.arm"));
  const skipped = await arm("skip", "alarm-skip");
  await host.deliver(skipped.alarm, "2026-11-20T14:00:00Z");
  assert.equal(decode(host.markdown(item)).messages.length, 0);
  const due = await arm("skip", "alarm-on-time");
  await host.deliver(due.alarm, "2026-11-20T13:00:01Z");
  assert.equal(decode(host.markdown(item)).messages.length, 2);
  const late = await arm("run", "alarm-run");
  assert.deepEqual(await arm("run", "alarm-run"), late);
  await host.deliver(late.alarm, "2026-11-20T14:00:00Z");
  assert.equal(host.deliver(late.alarm, "2026-11-20T14:00:00Z"), undefined);
  assert.deepEqual(decode(host.markdown(item)).messages.map(message => message.text), ["skip", "Reply to skip", "run", "Reply to run"]);
});

test("newest pages keep complete messages and older reads stay on the same revision", async () => {
  const { host, item, send, context } = await setup();
  host.claude = async text => text;
  const text = "🦊".repeat(3500);
  for (let index = 0; index < 3; index++) {
    await host.run((await send(`page-${index}`, text)).job_id);
  }
  const snapshot = host.snapshots.get(item)!;
  assert.ok(snapshot.older);
  assert.ok(Buffer.byteLength(JSON.stringify(snapshot.view)) < 65536);
  const read = (older?: string) => handlers["threads.refresh"]({
    item, source_revision: snapshot.source_revision, older,
  }, context("page-older", "threads.refresh"));
  let page = await read();
  assert.ok("view" in page);
  assert.deepEqual(page.view, snapshot.view);
  let transcript = "";
  while (true) {
    assert.ok("view" in page);
    const root: SnapshotNode = page.view.content;
    assert.equal(root.type, "stack");
    transcript += root.children.flatMap(node => node.type === "snapshot-text" ? [node.text] : []).join("");
    if (!page.older) break;
    page = await read(page.older);
  }
  assert.equal(transcript, text.repeat(6));
  assert.equal(host.snapshots.get(item), snapshot);
});

test("replay after a committed append with a lost ACK publishes the head without duplicating the reply", async () => {
  const { host, item, send } = await setup();
  let providerCalls = 0;
  host.claude = async () => {
    providerCalls++;
    return "Saved before disconnect";
  };
  const { job_id } = await send("lost-append-ack", "Run once");
  host.loseNextAppendAck = true;
  await assert.rejects(host.run(job_id), /acknowledgement lost/);
  const before = host.markdown(item);
  assert.notEqual(host.snapshots.get(item)?.source_revision, host.files.get(item)?.revision);
  await host.replay(job_id);
  assert.equal(providerCalls, 1);
  assert.equal(host.markdown(item), before);
  assert.equal(decode(before).messages.length, 2);
  assert.equal(host.snapshots.get(item)?.source_revision, host.files.get(item)?.revision);
});

test("unchecked harness and placement refs fail before job admission", async () => {
  const { host, item, selection, context } = await setup();
  const send = (input: Partial<typeof selection>) =>
    handlers["threads.send"]({ item, text: "Run this", ...selection, ...input }, context("forged-choice", "threads.send"));
  await assert.rejects(send({ harness: "claude-code" }), /unchecked harness/);
  await assert.rejects(send({ placement: "some-other-machine" }), /unchecked placement/);
  assert.equal(host.jobs.size, 0);
});

test("a rename while the harness is running preserves the reply and snapshot", async () => {
  const { host, item, send, context } = await setup();
  let reached: () => void;
  const started = new Promise<void>(resolve => { reached = resolve; });
  let release: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  host.claude = async () => { reached(); await gate; return "Finished"; };
  const run = host.run((await send("rename-in-flight", "Working")).job_id);
  await started;
  await handlers["threads.rename"]({ item, title: "New title" }, context("rename-in-flight-edit", "threads.rename"));
  release!();
  await run;
  const thread = decode(host.markdown(item));
  assert.equal(thread.title, "New title");
  assert.equal(thread.messages.at(-1)!.text, "Finished");
  assert.equal(host.snapshots.get(item)?.source_revision, host.files.get(item)?.revision);
});

test("two conversations with the same title get different readable placements", async () => {
  const { host, item, context } = await setup();
  const create = (request: string) =>
    handlers["threads.create"]({ title: "Review the lease" }, context(request, "threads.create"));
  const { created } = await create("same-title");
  assert.notEqual(created, item);
  assert.match(host.files.get(created)!.path!, /Review the lease \(2\)\/Conversation\.md$/);
  assert.deepEqual(await create("same-title"), { created });
  assert.equal(host.files.size, 2);
});

test("first contribution output and log survive Stop before the terminal frame", async () => {
  const { host, item, send, context } = await setup();
  let reached!: () => void;
  const outputCommitted = new Promise<void>(resolve => { reached = resolve; });
  let release!: () => void;
  const waitForStop = new Promise<void>(resolve => { release = resolve; });
  host.beforeTerminal = async () => { reached(); await waitForStop; };
  const { job_id } = await send("stop-after-output", "Say hello");
  const run = host.run(job_id);
  await outputCommitted;
  const reply = decode(host.markdown(item)).messages.at(-1)!;
  const source_revision = host.files.get(item)!.revision;
  const refresh = (anchor: string) => handlers["threads.refresh"]({ item, source_revision, anchor }, context("open-anchor", "threads.refresh"));
  assert.deepEqual(await refresh("unknown-message"), { anchor_unavailable: true });
  const anchored = await refresh(reply.id);
  assert.ok("anchor" in anchored);
  assert.equal(anchored.anchor, reply.id);
  assert.match(JSON.stringify(anchored.view), /Reply to Say hello/);
  host.stop(item, { execution: host.jobs.get(job_id)!.attempt!.execution });
  release();
  await assert.rejects(run, /attempt retired/);
  assert.equal(host.files.get(item)?.revision, source_revision);
  assert.match(host.markdown(reply.log!.id), /Reply to Say hello/);
});

test("a fresh checked harness binding reuses the stored logical agent participant", async () => {
  const { host, item, send, context } = await setup();
  await host.run((await send("original-binding", "First binding")).job_id);
  const previous = host.choose();
  const fresh = host.rotateHarness();
  assert.notEqual(previous.harness, fresh.harness);
  await assert.rejects(handlers["threads.send"]({ item, text: "Stale", ...previous },
    context("old-binding", "threads.send")), /unchecked harness/);
  const { job_id } = await handlers["threads.send"]({ item, text: "Second binding", ...fresh },
    context("new-binding", "threads.send"));
  await host.run(job_id);
  const agents = decode(host.markdown(item)).participants.filter(person => person.kind === "agent");
  assert.equal(agents.length, 1);
  assert.deepEqual(agents[0].contribution, { package: "calternal-threads-claude-code", contribution: "claude-code" });
  assert.doesNotMatch(host.markdown(item), /checked-harness/);
});

test("the shared run target is outside both Send and alarm broker payloads", async () => {
  const { host, item, send, selection, context } = await setup();
  const { job_id } = await send("single-send-item", "Start");
  assert.equal(host.jobs.get(job_id)?.context.item, item);
  await host.run(job_id);
  const { alarm } = await handlers["threads.arm"]({
    item, harness: selection.harness, text: "Later", at: "2026-11-21T12:00:00Z", if_missed: "run",
  }, context("single-alarm-item", "threads.arm"));
  assert.equal(host.alarms.get(alarm)?.item, item);
  assert.ok(!Object.hasOwn(host.alarms.get(alarm)!.input, "item"));
  await host.deliver(alarm, "2026-11-21T12:00:00Z");
  assert.match(host.markdown(item), /Reply to Later/);
});

test("Send and alarm admission refuse an oversized contribution with 413 before any write", async () => {
  const { host, item, send, selection, context } = await setup();
  let providerCalls = 0;
  host.claude = async () => { providerCalls++; return "Unexpected"; };
  const before = host.markdown(item);
  const revision = host.files.get(item)!.revision;
  const receipts = host.receipts.size;
  const error = { status: 413, code: "contribution_frame_limit" };
  const prompt = '"'.repeat(8192);
  await assert.rejects(send("oversized-send", prompt), error);
  await assert.rejects(handlers["threads.arm"]({
    item, harness: selection.harness, text: prompt, at: "2026-11-21T12:00:00Z", if_missed: "run",
  }, context("oversized-alarm", "threads.arm")), error);
  assert.equal(providerCalls, 0);
  assert.equal(host.jobs.size, 0);
  assert.equal(host.alarms.size, 0);
  assert.equal(host.receipts.size, receipts);
  assert.equal(host.markdown(item), before);
  assert.equal(host.files.get(item)!.revision, revision);
  assert.equal(host.snapshots.get(item)?.source_revision, revision);

  host.claude = async () => { providerCalls++; return "Fits"; };
  const accepted = await send("eligible-after-413", "Try again");
  assert.deepEqual(await send("eligible-after-413", "Try again"), accepted);
  await host.run(accepted.job_id);
  assert.equal(host.jobs.size, 1);
  assert.equal(providerCalls, 1);
});

test("the broker refuses a changed contribution input on the same admitted attempt", async () => {
  const { host, send } = await setup();
  let ready!: () => void;
  const atTerminal = new Promise<void>(resolve => { ready = resolve; });
  let release!: () => void;
  const pending = new Promise<void>(resolve => { release = resolve; });
  host.beforeTerminal = async () => { ready(); await pending; };
  const { job_id } = await send("pinned-input", "Approved");
  const running = host.run(job_id);
  await atTerminal;
  try {
    const job = host.jobs.get(job_id)!;
    assert.deepEqual(job.context.contribution_input, { text: "Approved" });
    const ctx = host.context(job_id, "threads.run", job);
    const substitute = { execution: job.attempt!.execution, contribution: job.context.harness, input: { text: "Changed" } };
    await assert.rejects(ctx.call("contribution_call", {
      ...substitute, request_id: step(job_id, "contribution"),
    }), /conflicting replay/);
    await assert.rejects(ctx.call("contribution_call", {
      ...substitute, request_id: "changed-call",
    }), /differs from admission/);
  } finally {
    release();
    await running;
  }
});

test("several checked prompts and agent replies preserve personal discovery", async () => {
  const { host, item, send } = await setup();
  const creator = host.files.get(item)!.creator;
  host.claude = async text => {
    assert.equal(host.writeAuthor(item), creator);
    assert.deepEqual(host.discoverAsAttempt(creator), [item]);
    return `Reply to ${text}`;
  };
  for (const [index, text] of ["First", "Second", "Third"].entries()) {
    const { job_id } = await send(`discovery-${index}`, text);
    await host.run(job_id);
    assert.equal(host.writeAuthor(item), creator);
    assert.deepEqual(host.discoverAsAttempt(creator), [item]);
  }
  const { messages } = decode(host.markdown(item));
  assert.deepEqual(messages.map(message => message.author.kind), ["person", "agent", "person", "agent", "person", "agent"]);
  assert.equal(messages.at(-1)?.text, "Reply to Third");
});

test("an unproven staged replacement sets consent_only and a worker cannot clear it", async () => {
  const { host, item, send } = await setup();
  const creator = "019a0000-0000-7000-8000-000000000001";
  const { job_id } = await send("before-import", "Keep the conversation searchable");
  await host.run(job_id);
  assert.deepEqual(host.discoverAsAttempt(creator), [item]);
  const body = Buffer.from(host.markdown(item) + "Imported material of unproven authorship\n" + "x".repeat(140 * 1024));
  const { begin, publish } = await host.stageUnproven(item, body);
  assert.deepEqual(host.discoverAsAttempt(creator), [item]);
  const receipt = await publish();
  assert.deepEqual(await begin(), receipt);
  assert.deepEqual(await publish(), receipt);
  await assert.rejects(begin(body.length - 1), /conflicting replay/);
  await assert.rejects(publish("Different save"), /conflicting replay/);
  assert.equal(host.markdown(item), body.toString());
  assert.equal(host.files.get(item)!.discovery, "consent_only");
  assert.equal(host.writeAuthor(item), undefined);
  assert.deepEqual(host.discoverAsAttempt(creator), []);
  const revision = host.files.get(item)!.revision;
  await assert.rejects(host.attemptRestoreDiscovery(item), /host-owned marker/);
  assert.equal(host.files.get(item)!.revision, revision);
  assert.deepEqual(host.discoverAsAttempt(creator), []);
});
