import type { SnapshotNode } from "../generated/wire.js";
import assert from "node:assert/strict";
import test, { after } from "node:test";
import { agentParticipant, attest, decode, decodeIndexed, encode, encodeMessage, messageRecord, personParticipant, step, type Thread } from "../src/format.ts";
import { FakeHost, handlers, checkTrace } from "./host.ts";

after(checkTrace);

async function setup() {
  const host = new FakeHost();
  const context = (request: string, operation: string) => host.context(request, operation);
  const { created: item } = await handlers["chat.create"]({ title: "Review the lease" }, host.createContext("create-1", "Review the lease"));
  const selection = host.choose();
  const send = (request: string, text: string) =>
    handlers["chat.send"]({ item, text, ...selection }, host.composerContext(request, item, text));
  return { host, item, send, selection, context };
}

test("create, select a contributed harness, append its reply and Stop an exact attempt", async () => {
  const { host, item, send, context } = await setup();
  assert.match(host.files.get(item)!.path!, /^Chats\/2026-11-20 Review the lease\/Conversation\.md$/);
  assert.deepEqual(host.thread(item).participants.map(p => p.kind), ["person"]);
  const first = await send("send-1", "Say hello 🦊\n\n---\n<!-- ## Agent -->");
  assert.deepEqual(await send("send-1", "Say hello 🦊\n\n---\n<!-- ## Agent -->"), first);
  await host.run(first.job_id);
  const thread = host.thread(item);
  const reply = thread.messages[1];
  assert.deepEqual(thread.participants.map(p => p.kind), ["person", "agent"]);
  assert.equal(reply.text, "Reply to Say hello 🦊\n\n---\n<!-- ## Agent -->");
  assert.equal(reply.replyQuote, thread.messages[0].text);
  assert.match(JSON.stringify(host.snapshots.get(item)!.view), /"timestamp":"2026-11-20T12:00:00Z"/);
  assert.match(host.markdown(reply.log!.id), /## Reply/);
  assert.ok(host.markdown(item).includes(`{#m-${reply.id} `));
  assert.equal((await handlers["chat.message"]({ item, message: reply.id }, context("read-1", "chat.message"))).text, reply.text);
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
  assert.deepEqual(host.thread(item).messages.map(message => message.text), ["First", "Second", "Reply to Second"]);
});

test("generic rename moves the folder, updates indexed links and preserves IDs and old read pins", async () => {
  const { host, item, send, context } = await setup();
  const original = host.snapshots.get(item)!;
  const { job_id } = await send("rename-head", "First");
  await host.run(job_id);
  const before = host.thread(item);
  const path = host.files.get(item)!.path;
  const anchor = `m-${before.messages[0].id}`;
  host.linkTo(item, "Notes/Meeting.md", anchor);
  host.linkTo(item, "Notes/Nested/Agenda.md", anchor);
  host.linkedMarkdown.set("Notes/Unrelated.md", "No conversation link\n");
  await host.rename(item, "Later title", "rename-1");
  const after = host.thread(item);
  assert.equal(after.title, "Later title");
  assert.equal(after.id, before.id);
  assert.deepEqual(after.messages, before.messages);
  assert.notEqual(host.files.get(item)!.path, path);
  assert.equal(host.files.get(item)!.path, "Chats/2026-11-20 Later title/Conversation.md");
  assert.equal(host.linkedMarkdown.get("Notes/Meeting.md"), `[Read the conversation](<../Chats/2026-11-20 Later title/Conversation.md#${anchor}>)\n`);
  assert.equal(host.linkedMarkdown.get("Notes/Nested/Agenda.md"), `[Read the conversation](<../../Chats/2026-11-20 Later title/Conversation.md#${anchor}>)\n`);
  assert.equal(host.linkedMarkdown.get("Notes/Unrelated.md"), "No conversation link\n");
  assert.deepEqual(await handlers["chat.refresh"]({
    item, source_revision: original.source_revision,
  }, context("refresh-1", "chat.refresh")), { view: original.view });
  assert.equal(host.snapshots.get(item)!.source_revision, host.files.get(item)!.revision);
  await host.rename(item, "Final title", "rename-2");
  assert.equal(host.linkedMarkdown.get("Notes/Meeting.md"), `[Read the conversation](<../Chats/2026-11-20 Final title/Conversation.md#${anchor}>)\n`);
  const moved = await host.rename(item, undefined, "move", undefined, ["Decisions"]);
  if (moved.state !== "ok") throw new Error("Expected inline move");
  assert.equal(host.linkedMarkdown.get("Notes/Meeting.md"), `[Read the conversation](<../Chats/Decisions/2026-11-20 Final title/Conversation.md#${anchor}>)\n`);
  await host.undoRename(moved.undo.ref, "undo-move");
  assert.equal(host.linkedMarkdown.get("Notes/Meeting.md"), `[Read the conversation](<../Chats/2026-11-20 Final title/Conversation.md#${anchor}>)\n`);
  const final = host.files.get(item)!;
  const spanIndex = final.spanHistory.get(final.revision)!;
  assert.deepEqual(decodeIndexed(Buffer.from(host.markdown(item)), spanIndex.spans, spanIndex.body_provenance).thread.messages, before.messages);
  assert.deepEqual(spanIndex.spans[0]?.attribution, { kind: "host" });
  assert.deepEqual(spanIndex.spans.at(-1)?.quote?.attribution, { kind: "person", actor: final.creator });

  const pending = await send("pre-archive", "Never start");
  const queued = await send("pre-archive-queued", "Never promote");
  host.archive(item);
  assert.equal(host.jobs.get(queued.job_id)!.state, "stopped");
  assert.equal(host.jobs.get(pending.job_id)!.state, "stopped");
  await assert.rejects(send("post-archive", "Never admit"), /target unavailable/);
  assert.equal(host.thread(item).messages.length, 2);
});

test("rename commits the title and folder before deferred links finish and old links still follow the item", async () => {
  const { host, item, send } = await setup();
  await host.run((await send("before-relocation", "First")).job_id);
  const path = host.files.get(item)!.path!;
  const anchor = `m-${host.thread(item).messages[0].id}`;
  host.linkTo(item, "Notes/First.md", anchor);
  host.linkTo(item, "Notes/Later.md", anchor);
  host.inlineBacklinkLimit = 1;
  const later = host.linkedMarkdown.get("Notes/Later.md");
  const outcome = await host.rename(item, "Deferred title", "rename-with-overflow");
  if (outcome.state !== "pending") throw new Error("Expected deferred rewrites");
  const expected = "Chats/2026-11-20 Deferred title/Conversation.md";
  assert.notEqual(path, expected);
  assert.equal(host.files.get(item)?.path, expected);
  assert.equal(host.thread(item).title, "Deferred title");
  assert.match(host.linkedMarkdown.get("Notes/First.md")!, /Deferred title/);
  assert.equal(host.linkedMarkdown.get("Notes/Later.md"), later);
  assert.deepEqual(host.followLink("Notes/Later.md"), { item, path: expected, anchor });
  assert.throws(() => host.followLink("Notes/Later.md", "outsider"), /unavailable/);
  assert.ok(!("undo" in outcome));
  const complete = host.finishRelocation(outcome.status_ref);
  assert.equal(complete.state, "ok");
  assert.ok(complete.undo.ref);
  assert.match(host.linkedMarkdown.get("Notes/Later.md")!, /Deferred title/);
  assert.throws(() => host.finishRelocation(outcome.status_ref), /unavailable/);
});

test("alarm delivery retains checked harness and lets the worker decide whether to run late", async () => {
  const { host, item, selection, context } = await setup();
  const arm = (if_missed: "skip" | "run", request: string) =>
    handlers["chat.arm"]({
      item, harness: selection.harness, text: if_missed, at: "2026-11-20T13:00:00Z", if_missed,
    }, context(request, "chat.arm"));
  const skipped = await arm("skip", "alarm-skip");
  await host.deliver(skipped.alarm, "2026-11-20T13:01:01Z");
  assert.equal(host.thread(item).messages.length, 0);
  const due = await arm("skip", "alarm-on-time");
  assert.equal(host.alarms.get(due.alarm)?.grace_seconds, 60);
  await host.deliver(due.alarm, "2026-11-20T13:01:00Z");
  assert.equal(host.thread(item).messages.length, 2);
  const late = await arm("run", "alarm-run");
  assert.deepEqual(await arm("run", "alarm-run"), late);
  await host.deliver(late.alarm, "2026-11-20T14:00:00Z");
  assert.equal(host.deliver(late.alarm, "2026-11-20T14:00:00Z"), undefined);
  assert.deepEqual(host.thread(item).messages.map(message => message.text), ["skip", "Reply to skip", "run", "Reply to run"]);
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
  const read = (older?: string) => handlers["chat.refresh"]({
    item, source_revision: snapshot.source_revision, older,
  }, context("page-older", "chat.refresh"));
  let page = await read();
  assert.ok("view" in page);
  assert.deepEqual(page.view, snapshot.view);
  let transcript = "";
  while (true) {
    assert.ok(page.view);
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
    host.loseNextAppendAck = true;
    return "Saved before disconnect";
  };
  const { job_id } = await send("lost-append-ack", "Run once");
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
    handlers["chat.send"]({ item, text: "Run this", ...selection, ...input }, context("forged-choice", "chat.send"));
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
  await host.rename(item, "New title", "rename-in-flight-edit");
  release!();
  await run;
  const thread = host.thread(item);
  assert.equal(thread.title, "New title");
  assert.equal(thread.messages.at(-1)!.text, "Finished");
  assert.equal(host.snapshots.get(item)?.source_revision, host.files.get(item)?.revision);
});

test("two conversations with the same title get different readable placements", async () => {
  const { host, item, context } = await setup();
  const create = (request: string) =>
    handlers["chat.create"]({ title: "Review the lease" }, host.createContext(request, "Review the lease"));
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
  const reply = host.thread(item).messages.at(-1)!;
  const source_revision = host.files.get(item)!.revision;
  const refresh = (anchor: string) => handlers["chat.refresh"]({ item, source_revision, anchor }, context("open-anchor", "chat.refresh"));
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
  await assert.rejects(handlers["chat.send"]({ item, text: "Stale", ...previous },
    context("old-binding", "chat.send")), /unchecked harness/);
  const { job_id } = await handlers["chat.send"]({ item, text: "Second binding", ...fresh },
    host.composerContext("new-binding", item, "Second binding"));
  await host.run(job_id);
  const agents = host.thread(item).participants.filter(person => person.kind === "agent");
  assert.equal(agents.length, 1);
  assert.deepEqual(agents[0].contribution, { package: "calternal-chat-claude-code", contribution: "claude-code" });
  assert.doesNotMatch(host.markdown(item), /checked-harness/);
});

test("the shared run target is outside both Send and alarm broker payloads", async () => {
  const { host, item, send, selection, context } = await setup();
  const { job_id } = await send("single-send-item", "Start");
  assert.equal(host.jobs.get(job_id)?.context.item, item);
  await host.run(job_id);
  const { alarm } = await handlers["chat.arm"]({
    item, harness: selection.harness, text: "Later", at: "2026-11-21T12:00:00Z", if_missed: "run",
  }, context("single-alarm-item", "chat.arm"));
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
  await assert.rejects(handlers["chat.arm"]({
    item, harness: selection.harness, text: prompt, at: "2026-11-21T12:00:00Z", if_missed: "run",
  }, context("oversized-alarm", "chat.arm")), error);
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
    const ctx = host.context(job_id, "chat.run", job);
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

test("the creator retains discovery across several attested prompts and replies", async () => {
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
  const { messages } = host.thread(item);
  assert.deepEqual(messages.map(message => message.author.kind), ["person", "agent", "person", "agent", "person", "agent"]);
  assert.equal(messages.at(-1)?.text, "Reply to Third");
});

test("the shorter heading and suffix fit a long reply and keep a literal delimiter attested", async () => {
  const { host, item, send, context } = await setup();
  const agent = { ...agentParticipant("test-thread", { package: "calternal-chat-claude-code", contribution: "claude-code" }),
    name: "é".repeat(100) };
  const base = { id: "0123456789abcdef", author: agent, posted_at: "2026-11-20T12:00:00Z",
    replyTo: "aabbccddeeff0011", replyToName: "Alice", text: "Reply",
    log: { space: "019a0000-0000-7000-8000-000000000010", id: "run-1" } };
  const heading = messageRecord(base, [agent], true);
  assert.ok(Buffer.byteLength(heading.prefix) <= 512);
  assert.doesNotMatch(heading.prefix, /log=/);
  assert.match(heading.suffix, /<!-- run-log=019a0000-0000-7000-8000-000000000010\/run-1 -->/);
  assert.ok(attest(heading, { kind: "output", seq: 1 }));
  const oversized = messageRecord({ ...base, replyToName: "🐝".repeat(100) }, [agent], true);
  assert.ok(Buffer.byteLength(oversized.prefix) > 512);
  assert.equal(attest(oversized, { kind: "output", seq: 1 }), undefined);

  const delimiter = "### Example {#m-aabbccddeeff0011 by=p1}";
  const { job_id } = await send("escaped-heading", delimiter);
  await host.run(job_id);
  assert.ok(host.markdown(item).includes(`\n${delimiter}\n\n`));
  assert.ok(!host.markdown(item).includes(`\\${delimiter}`));
  const index = host.files.get(item)!.spanHistory.get(host.files.get(item)!.revision)!;
  assert.deepEqual(index.spans.map(span => span.attribution?.kind), ["person", "person", "attempt"]);
  const first = decodeIndexed(Buffer.from(host.markdown(item)), index.spans, index.body_provenance).thread.messages[0];
  const message = await handlers["chat.message"]({ item, message: first.id }, context("reopen-literal", "chat.message"));
  assert.equal(message.text, delimiter);
  assert.equal(host.files.get(item)!.discovery, "creator_only");
});

test("a checked editor save qualifies complete bytes and downgrades unexplained bytes in the same checked path", async () => {
  const host = new FakeHost();
  const { created: item } = await handlers["chat.create"]({ title: "Field notes" }, host.createContext("editor-source", "Field notes"));
  const creator = host.files.get(item)!.creator;
  const snapshot = "---\ntitle: Field notes\n---\n\n# Field notes\n\n🐝 by the gate.\n";
  await host.saveCapturedText(item, snapshot, Buffer.from(snapshot));
  assert.equal(host.writeAuthor(item), creator);
  assert.deepEqual(host.discoverAsAttempt(creator), [item]);

  await host.saveCapturedText(item, snapshot, Buffer.from(snapshot + "\nImported material\n"));
  assert.equal(host.writeAuthor(item), undefined);
  assert.deepEqual(host.discoverAsAttempt(creator), []);
});

test("Markdown keeps one plain body span and reversibly escapes only tagged heading lines", () => {
  const alice = personParticipant("019a0000-0000-7000-8000-000000000001", "Alice");
  const posted_at = "2026-11-20T12:00:00Z";
  const id = "0123456789abcdef";
  const reserved = "### Looks like a record {#m-aabbccddeeff0011 by=p1}";
  const plain = "\nBring 🐝 gloves.\r\n\n### Ordinary heading\n> Quoted by the author\n";
  const text = `${plain}${reserved}\n\\${reserved}\n`;
  const message = { author: alice, id, posted_at, text };
  const thread: Thread = { id: "0".repeat(32), title: "Garden", created: posted_at, participants: [alice], messages: [] };

  const prefix = "### Alice · 2026-11-20 12:00 UTC {#m-0123456789abcdef by=p1}\n\n";
  const expected = Buffer.from(`${prefix}${plain}\\${reserved}\n\\\\${reserved}\n\n\n`);
  assert.equal(encodeMessage({ ...message, text: plain }, [alice]), `${prefix}${plain}\n\n`);
  const record = Buffer.from(encodeMessage(message, [alice]));
  assert.deepEqual(record, expected);
  assert.equal(record.includes(Buffer.from(text)), false);

  const existing = Buffer.from(encode(thread));
  const saved = encode({ ...thread, messages: [message] });
  assert.deepEqual(Buffer.from(saved), Buffer.concat([existing, expected]));
  assert.deepEqual(decode(saved).messages, [message]);
  const agent = agentParticipant(thread.id, { package: "calternal-chat-claude-code", contribution: "claude-code" });
  const agentHeader = Buffer.from(
    `  - p2 | Claude Code | agent | ${agent.id}\nagent-p2-plugin: calternal-chat-claude-code\nagent-p2-harness: claude-code\n`,
  );
  const withFirstPrompt = Buffer.from(encode({ ...thread, participants: [alice, agent], messages: [message] }));
  assert.deepEqual(withFirstPrompt, Buffer.concat([existing.subarray(0, -5), agentHeader, existing.subarray(-5), expected]));
  const graft = Buffer.concat([record, Buffer.from("\nImported material\n")]);
  assert.equal(graft.includes(expected), true);
  assert.equal(graft.equals(expected), false);
});

test("indexed spans preserve literal heading and trailer lookalikes next to an opaque record", () => {
  const alice = personParticipant("019a0000-0000-7000-8000-000000000001", "Alice 🐝");
  const posted_at = "2026-11-20T12:00:00Z";
  const thread: Thread = { id: "0".repeat(32), title: "Garden", created: posted_at, participants: [alice], messages: [] };
  const header = Buffer.from(encode(thread));
  const tagged = "### Looks like a record {#m-aabbccddeeff0011 by=p1}";
  const first = { author: alice, id: "0123456789abcdef", posted_at,
    text: `Quoted literally:\n${tagged}\n\\${tagged}\n\n<!-- run-log=example/body -->`,
    log: { space: "019a0000-0000-7000-8000-000000000010", id: "run-1" } };
  const { prefix, text, suffix } = messageRecord(first, [alice]);
  const record = Buffer.from(prefix + text + suffix);
  const next = { author: alice, id: "fedcba9876543210", posted_at, text: tagged };
  const opaque = Buffer.from(encodeMessage(next, [alice]));
  const offset = header.length + opaque.length;
  const result = decodeIndexed(Buffer.concat([header, opaque, record]), [
    { offset, length: record.length, span: "first-message", middle_offset: offset + Buffer.byteLength(prefix),
      middle_length: Buffer.byteLength(text), attribution: { kind: "person", actor: alice.id } },
  ], "unproven");
  assert.deepEqual(result.thread.messages, [next, first]);
  assert.deepEqual([...result.spans], [[first.id, "first-message"]]);
});

test("a checked read joins host-origin range chunks inside one recorded message", async () => {
  const { host, item, send, context } = await setup();
  const job = await send("origin-chunks", "link A and link B");
  await host.run(job.job_id);
  const file = host.files.get(item)!;
  const { spans } = file.spanHistory.get(file.revision)!;
  const [title, prompt, reply] = spans;
  assert.equal(prompt.attribution?.kind, "person");
  assert.equal(reply.attribution?.kind, "attempt");
  const message = host.thread(item).messages[0].id;
  const ranges = [{ offset: prompt.middle_offset + 5, length: 1 }, { offset: prompt.middle_offset + 16, length: 1 }];
  spans.splice(0, spans.length, title, { ...prompt, host_ranges: [ranges[0]], host_ranges_more: true },
    { ...prompt, host_ranges: [ranges[1]] }, reply);
  host.spanPageSize = 2;
  const result = await handlers["chat.message"]({ item, message }, context("read-origin-chunks", "chat.message"));
  assert.equal(result.text, "link A and link B");
  spans[2].host_ranges![0].offset = ranges[0].offset;
  await assert.rejects(handlers["chat.message"]({ item, message }, context("overlapping-origin-chunks", "chat.message")),
    /host-written ranges are invalid/);
});

test("opaque fallback stages a large reframe without losing formerly indexed literal headings", async () => {
  const { host, item, send, selection, context } = await setup();
  const delimiter = "### Ordinary text {#m-aabbccddeeff0011 by=p1}";
  host.claude = async () => "x".repeat(12_000);
  for (let index = 0; index < 12; index++) {
    const { job_id } = await send(`long-${index}`, delimiter);
    await host.run(job_id);
  }
  const file = host.files.get(item)!;
  const oldRevision = file.revision;
  const old = file.spanHistory.get(oldRevision)!;
  const first = decodeIndexed(Buffer.from(host.markdown(item)), old.spans, old.body_provenance).thread.messages[0];
  assert.ok(Buffer.byteLength(host.markdown(item)) > 128 * 1024);
  const alarm = await handlers["chat.arm"]({ item, harness: selection.harness, text: "Scheduled",
    at: "2026-11-20T13:00:00Z", if_missed: "run" }, context("arm-fallback", "chat.arm"));
  await host.deliver(alarm.alarm, "2026-11-20T13:00:01Z");
  assert.equal(file.discovery, "consent_only");
  assert.equal(file.spanHistory.get(file.revision)?.spans.length, 0);
  assert.equal(file.spanHistory.get(oldRevision)?.spans.length, 25);
  assert.ok(host.markdown(item).includes(`\\${delimiter}`));
  const result = await handlers["chat.message"]({ item, message: first.id }, context("read-after-fallback", "chat.message"));
  assert.equal(result.text, first.text);
  await host.run((await send("after-opaque", "New checked message")).job_id);
  assert.equal(file.spanHistory.get(file.revision)?.spans.length, 2);
  assert.equal(file.discovery, "consent_only");
});

test("an unproven staged replacement sets consent_only and a worker cannot clear it", async () => {
  const { host, item, send } = await setup();
  const { job_id } = await send("before-import", "Proven conversation");
  await host.run(job_id);
  const prior = host.files.get(item)!.revision;
  const creator = "019a0000-0000-7000-8000-000000000001";
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
  const file = host.files.get(item)!;
  assert.deepEqual(file.spanHistory.get(file.revision), { body_provenance: "unproven", spans: [] });
  assert.equal(file.spanHistory.get(prior)?.spans.length, 3);
  assert.deepEqual(host.discoverAsAttempt(creator), []);
  const revision = host.files.get(item)!.revision;
  await assert.rejects(host.attemptRestoreDiscovery(item), /host-owned marker/);
  assert.equal(host.files.get(item)!.revision, revision);
  assert.deepEqual(host.discoverAsAttempt(creator), []);
});
