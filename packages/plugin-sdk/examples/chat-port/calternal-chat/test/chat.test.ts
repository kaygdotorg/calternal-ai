import assert from "node:assert/strict";
import test from "node:test";
import { saveBody, subeffectKey } from "@calternal/plugin-sdk";
import type { ItemViewPage, SnapshotNode } from "@calternal/plugin-sdk/types";
import { agentParticipant, attest, decode, decodeIndexed, encode, encodeMessage, messageRecord, personParticipant, type Thread } from "../src/format.ts";
import { setup } from "./host.ts";

type Message = { text: string };
const gate = () => {
  let open!: () => void;
  return { wait: new Promise<void>(resolve => { open = resolve; }), open: () => open() };
};

test("create, select a contributed harness, append its reply and Stop an exact attempt", async () => {
  const { host, item, send } = await setup();
  assert.match(host.item(item).path, /^Chats\/2026-11-20 Review the lease\/Conversation\.md$/);
  assert.deepEqual(host.thread(item).participants.map(p => p.kind), ["person"]);
  const first = await send("send-1", "Say hello 🦊\n\n---\n<!-- ## Agent -->");
  assert.deepEqual(await send("send-1", "Say hello 🦊\n\n---\n<!-- ## Agent -->"), first);
  await host.run(first.job_id);
  const thread = host.thread(item);
  const reply = thread.messages[1];
  assert.deepEqual(thread.participants.map(p => p.kind), ["person", "agent"]);
  assert.equal(reply.text, "Reply to Say hello 🦊\n\n---\n<!-- ## Agent -->");
  assert.equal(reply.replyQuote, thread.messages[0].text);
  assert.match(JSON.stringify(host.view(item, "body")!.view), /"timestamp":"2026-11-20T12:00:00Z"/);
  assert.match(host.text(reply.log!.id), /## Reply/);
  assert.ok(host.text(item).includes(`{#m-${reply.id} `));
  assert.equal((await host.invoke("chat.message", { item, message: reply.id }) as Message).text, reply.text);
  assert.equal(host.view(item, "body")?.source_revision, host.item(item).head);
  assert.match(JSON.stringify(host.view(item, "body")!.view), /Reply to Say hello/);

  const reached = gate();
  const release = gate();
  host.claude = async () => { reached.open(); await release.wait; return "late reply"; };
  const second = await send("send-2", "Keep working");
  const run = host.run(second.job_id);
  await reached.wait;
  assert.throws(() => host.stop(item, { execution: host.jobs.get(first.job_id)!.attempt!.execution }), /stale Stop/);
  host.stop(item, { execution: host.jobs.get(second.job_id)!.attempt!.execution });
  release.open();
  await assert.rejects(run, /attempt retired/);
  assert.doesNotMatch(host.text(item), /late reply/);
  const logs = [...host.items.items.values()].filter(file => file.package === "calternal-chat-claude-code");
  assert.equal(logs.length, 2);
  assert.match(host.text(logs[1].id), /## Prompt\n\nKeep working/);
  assert.doesNotMatch(host.text(logs[1].id), /## Reply/);
});

test("one durable successor waits for the head and exact attempt teardown", async () => {
  const { host, item, send } = await setup();
  const first = await send("queue-head", "First");
  const queued = await send("queue-next", "Second");
  assert.deepEqual(await send("queue-next", "Second"), queued);
  await assert.rejects(send("queue-full", "Third"), /pending_limit/);
  await assert.rejects(host.run(queued.job_id), /successor must wait/);
  const reached = gate();
  const release = gate();
  host.claude = async () => { reached.open(); await release.wait; return "retired"; };
  const run = host.run(first.job_id);
  await reached.wait;
  host.stop(item, { execution: host.jobs.get(first.job_id)!.attempt!.execution });
  await assert.rejects(host.run(queued.job_id), /successor must wait for teardown/);
  release.open();
  await assert.rejects(run, /attempt retired/);
  host.settle(first.job_id);
  host.claude = async text => `Reply to ${text}`;
  await host.run(queued.job_id);
  assert.deepEqual(host.thread(item).messages.map(message => message.text), ["First", "Second", "Reply to Second"]);
});

test("archive stops pending and queued jobs and refuses later admission", async () => {
  const { host, item, send } = await setup();
  await host.run((await send("before-archive", "First")).job_id);
  const pending = await send("pre-archive", "Never start");
  const queued = await send("pre-archive-queued", "Never promote");
  host.archive(item);
  assert.equal(host.jobs.get(queued.job_id)!.state, "stopped");
  assert.equal(host.jobs.get(pending.job_id)!.state, "stopped");
  await assert.rejects(send("post-archive", "Never admit"), /target unavailable/);
  assert.equal(host.thread(item).messages.length, 2);
});

test("alarm delivery retains checked harness and lets the worker decide whether to run late", async () => {
  const { host, item, selection } = await setup();
  const arm = (if_missed: "skip" | "run", request_id: string) =>
    host.invoke("chat.arm", { item, harness: selection.harness, text: if_missed, at: "2026-11-20T13:00:00Z", if_missed }, { request_id }) as Promise<{ alarm: string }>;
  const skipped = await arm("skip", "alarm-skip");
  await host.deliver(skipped.alarm, "2026-11-20T13:01:01Z");
  assert.equal(host.thread(item).messages.length, 0);
  const due = await arm("skip", "alarm-on-time");
  assert.equal(host.execution.alarms.get(due.alarm)?.set.grace_seconds, 60);
  await host.deliver(due.alarm, "2026-11-20T13:01:00Z");
  assert.equal(host.thread(item).messages.length, 2);
  const late = await arm("run", "alarm-run");
  assert.deepEqual(await arm("run", "alarm-run"), late);
  await host.deliver(late.alarm, "2026-11-20T14:00:00Z");
  assert.equal(await host.deliver(late.alarm, "2026-11-20T14:00:00Z"), undefined);
  assert.deepEqual(host.thread(item).messages.map(message => message.text), ["skip", "Reply to skip", "run", "Reply to run"]);
});

test("newest pages keep complete messages and older reads stay on the same revision", async () => {
  const { host, item, send } = await setup();
  host.claude = async text => text;
  const text = "🦊".repeat(3500);
  for (let index = 0; index < 3; index++) {
    await host.run((await send(`page-${index}`, text)).job_id);
  }
  const snapshot = host.view(item, "body")!;
  assert.ok(snapshot.older);
  assert.ok(Buffer.byteLength(JSON.stringify(snapshot.view)) < 65536);
  const read = (older?: string) => host.invoke("chat.refresh", { item, source_revision: snapshot.source_revision, ...(older && { older }) }) as Promise<ItemViewPage>;
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
  assert.deepEqual(host.view(item, "body"), snapshot);
});

test("replay after a committed append with a lost ACK publishes the head without duplicating the reply", async () => {
  const { host, item, send } = await setup();
  let providerCalls = 0;
  host.claude = async () => {
    providerCalls++;
    host.failNext("item_append", "lose_ack", "calternal-chat");
    return "Saved before disconnect";
  };
  const { job_id } = await send("lost-append-ack", "Run once");
  await assert.rejects(host.run(job_id), /acknowledgement lost/);
  const before = host.text(item);
  assert.notEqual(host.view(item, "body")?.source_revision, host.item(item).head);
  await host.replay(job_id);
  assert.equal(providerCalls, 1);
  assert.equal(host.text(item), before);
  assert.equal(decode(before).messages.length, 2);
  assert.equal(host.view(item, "body")?.source_revision, host.item(item).head);
});

test("unchecked harness and placement refs fail before job admission", async () => {
  const { host, item, selection } = await setup();
  const send = (input: Partial<typeof selection>) => host.invoke("chat.send", { item, text: "Run this", ...selection, ...input });
  await assert.rejects(send({ harness: "claude-code" }), /unchecked harness/);
  await assert.rejects(send({ placement: "some-other-machine" }), /unchecked placement/);
  assert.equal(host.jobs.size, 0);
});

test("two conversations with the same title get different readable placements", async () => {
  const { host, item } = await setup();
  const create = (request_id: string) => host.invoke("chat.create", { title: "Review the lease" }, { captured: true, request_id }) as Promise<{ created: string }>;
  const { created } = await create("same-title");
  assert.notEqual(created, item);
  assert.match(host.item(created).path, /Review the lease \(2\)\/Conversation\.md$/);
  assert.deepEqual(await create("same-title"), { created });
  assert.equal([...host.items.items.values()].filter(file => file.kind.id === "chats").length, 2);
});

test("first contribution output and log survive Stop before the terminal frame", async () => {
  const { host, item, send } = await setup();
  const committed = gate();
  const stopped = gate();
  host.execution.beforeTerminal = async () => { committed.open(); await stopped.wait; };
  const { job_id } = await send("stop-after-output", "Say hello");
  const run = host.run(job_id);
  await committed.wait;
  // The output frame is committed; wait until the worker has written it.
  while (host.thread(item).messages.length < 2) await new Promise(resolve => setTimeout(resolve, 1));
  const reply = host.thread(item).messages.at(-1)!;
  const source_revision = host.item(item).head;
  const refresh = (anchor: string) => host.invoke("chat.refresh", { item, source_revision, anchor }) as Promise<ItemViewPage>;
  assert.deepEqual(await refresh("unknown-message"), { anchor_unavailable: true });
  const anchored = await refresh(reply.id);
  assert.ok("anchor" in anchored);
  assert.equal(anchored.anchor, reply.id);
  assert.match(JSON.stringify(anchored.view), /Reply to Say hello/);
  host.stop(item, { execution: host.jobs.get(job_id)!.attempt!.execution });
  stopped.open();
  await assert.rejects(run, /attempt retired/);
  assert.equal(host.item(item).head, source_revision);
  assert.match(host.text(reply.log!.id), /Reply to Say hello/);
});

test("a fresh checked harness binding reuses the stored logical agent participant", async () => {
  const { host, item, send, selection: previous } = await setup();
  await host.run((await send("original-binding", "First binding")).job_id);
  const fresh = host.choose();
  assert.notEqual(previous.harness, fresh.harness);
  await assert.rejects(host.invoke("chat.send", { item, text: "Stale", ...previous }), /unchecked harness/);
  const { job_id } = await host.invoke("chat.send", { item, text: "Second binding", ...fresh }, { captured: true, request_id: "new-binding" }) as { job_id: string };
  await host.run(job_id);
  const agents = host.thread(item).participants.filter(person => person.kind === "agent");
  assert.equal(agents.length, 1);
  assert.deepEqual(agents[0].contribution, { package: "calternal-chat-claude-code", contribution: "claude-code" });
  assert.doesNotMatch(host.text(item), /harness-/);
});

test("the shared run target is outside both Send and alarm broker payloads", async () => {
  const { host, item, send, selection } = await setup();
  const { job_id } = await send("single-send-item", "Start");
  assert.equal(host.jobs.get(job_id)?.item, item);
  await host.run(job_id);
  const { alarm } = await host.invoke("chat.arm", { item, harness: selection.harness, text: "Later", at: "2026-11-21T12:00:00Z", if_missed: "run" }) as { alarm: string };
  assert.equal(host.execution.alarms.get(alarm)?.set.item, item);
  assert.ok(!Object.hasOwn(host.execution.alarms.get(alarm)!.set.input, "item"));
  await host.deliver(alarm, "2026-11-21T12:00:00Z");
  assert.match(host.text(item), /Reply to Later/);
});

test("Send and alarm admission refuse an oversized contribution with 413 before any write", async () => {
  const { host, item, send, selection } = await setup();
  let providerCalls = 0;
  host.claude = async () => { providerCalls++; return "Unexpected"; };
  const before = host.text(item);
  const revision = host.item(item).head;
  const receipts = host.receipts.size;
  const error = { status: 413, code: "contribution_frame_limit" };
  const prompt = '"'.repeat(8192);
  await assert.rejects(send("oversized-send", prompt), error);
  await assert.rejects(host.invoke("chat.arm", { item, harness: selection.harness, text: prompt, at: "2026-11-21T12:00:00Z", if_missed: "run" }), error);
  assert.equal(providerCalls, 0);
  assert.equal(host.jobs.size, 0);
  assert.equal(host.execution.alarms.size, 0);
  assert.equal(host.receipts.size, receipts);
  assert.equal(host.text(item), before);
  assert.equal(host.item(item).head, revision);
  assert.equal(host.view(item, "body")?.source_revision, revision);

  host.claude = async () => { providerCalls++; return "Fits"; };
  const accepted = await send("eligible-after-413", "Try again");
  assert.deepEqual(await send("eligible-after-413", "Try again"), accepted);
  await host.run(accepted.job_id);
  assert.equal(host.jobs.size, 1);
  assert.equal(providerCalls, 1);
});

test("the broker refuses a changed contribution input on the same admitted attempt", async () => {
  const { host, send } = await setup();
  const atTerminal = gate();
  const pending = gate();
  host.execution.beforeTerminal = async () => { atTerminal.open(); await pending.wait; };
  const { job_id } = await send("pinned-input", "Approved");
  const running = host.run(job_id);
  await atTerminal.wait;
  try {
    const job = host.jobs.get(job_id)!;
    assert.deepEqual(job.contributionInput, { text: "Approved" });
    const ctx = host.context<"contribution_call">("chat.run", { job: job_id });
    const substitute = { execution: job.attempt!.execution, contribution: job.harness, input: { text: "Changed" } };
    await assert.rejects(ctx.call("contribution_call", { ...substitute, request_id: subeffectKey(job_id, "contribution") }), /conflicting replay/);
    await assert.rejects(ctx.call("contribution_call", { ...substitute, request_id: "changed-call" }), /differs from admission/);
  } finally {
    pending.open();
    await running;
  }
});

test("the creator retains discovery across several attested prompts and replies", async () => {
  const { host, item, send } = await setup();
  const creator = host.item(item).creator;
  host.claude = async text => {
    assert.equal(host.item(item).author, creator);
    assert.deepEqual(host.discoverable(creator), [item]);
    return `Reply to ${text}`;
  };
  for (const [index, text] of ["First", "Second", "Third"].entries()) {
    const { job_id } = await send(`discovery-${index}`, text);
    await host.run(job_id);
    assert.equal(host.item(item).author, creator);
    assert.deepEqual(host.discoverable(creator), [item]);
  }
  const { messages } = host.thread(item);
  assert.deepEqual(messages.map(message => message.author.kind), ["person", "agent", "person", "agent", "person", "agent"]);
  assert.equal(messages.at(-1)?.text, "Reply to Third");
});

test("the shorter heading and suffix fit a long reply and keep a literal delimiter attested", async () => {
  const { host, item, send } = await setup();
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
  assert.ok(host.text(item).includes(`\n${delimiter}\n\n`));
  assert.ok(!host.text(item).includes(`\\${delimiter}`));
  const stored = host.item(item);
  const index = stored.index.get(stored.head)!;
  assert.deepEqual(index.spans.map(span => span.attribution?.kind), ["person", "person", "attempt"]);
  const first = decodeIndexed(Buffer.from(host.text(item)), index.spans, index.body_provenance).thread.messages[0];
  const message = await host.invoke("chat.message", { item, message: first.id }) as Message;
  assert.equal(message.text, delimiter);
  assert.equal(stored.discovery, "creator_only");
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
  const { host, item, send } = await setup();
  const job = await send("origin-chunks", "link A and link B");
  await host.run(job.job_id);
  const stored = host.item(item);
  const { spans } = stored.index.get(stored.head)!;
  const [title, prompt, reply] = spans;
  assert.equal(prompt.attribution?.kind, "person");
  assert.equal(reply.attribution?.kind, "attempt");
  const message = host.thread(item).messages[0].id;
  const ranges = [{ offset: prompt.middle_offset + 5, length: 1 }, { offset: prompt.middle_offset + 16, length: 1 }];
  spans.splice(0, spans.length, title, { ...prompt, host_ranges: [ranges[0]], host_ranges_more: true },
    { ...prompt, host_ranges: [ranges[1]] }, reply);
  host.items.spanPageSize = 2;
  const result = await host.invoke("chat.message", { item, message }) as Message;
  assert.equal(result.text, "link A and link B");
  spans[2].host_ranges![0].offset = ranges[0].offset;
  await assert.rejects(host.invoke("chat.message", { item, message }), /host-written ranges are invalid/);
});

test("opaque fallback stages a large reframe without losing formerly indexed literal headings", async () => {
  const { host, item, send, selection } = await setup();
  const delimiter = "### Ordinary text {#m-aabbccddeeff0011 by=p1}";
  host.claude = async () => "x".repeat(12_000);
  for (let index = 0; index < 12; index++) {
    const { job_id } = await send(`long-${index}`, delimiter);
    await host.run(job_id);
  }
  const stored = host.item(item);
  const oldRevision = stored.head;
  const old = stored.index.get(oldRevision)!;
  const first = decodeIndexed(Buffer.from(host.text(item)), old.spans, old.body_provenance).thread.messages[0];
  assert.ok(Buffer.byteLength(host.text(item)) > 128 * 1024);
  const alarm = await host.invoke("chat.arm", { item, harness: selection.harness, text: "Scheduled", at: "2026-11-20T13:00:00Z", if_missed: "run" }) as { alarm: string };
  await host.deliver(alarm.alarm, "2026-11-20T13:00:01Z");
  assert.equal(stored.discovery, "consent_only");
  assert.equal(stored.index.get(stored.head)?.spans.length, 0);
  assert.equal(stored.index.get(oldRevision)?.spans.length, 25);
  assert.ok(host.text(item).includes(`\\${delimiter}`));
  assert.ok(host.calls.some(call => call.package === "calternal-chat" && call.method === "item_save_begin"));
  const result = await host.invoke("chat.message", { item, message: first.id }) as Message;
  assert.equal(result.text, first.text);
  await host.run((await send("after-opaque", "New checked message")).job_id);
  assert.equal(stored.index.get(stored.head)?.spans.length, 2);
  assert.equal(stored.discovery, "consent_only");
});

test("an unproven staged replacement sets consent_only and a worker cannot clear it", async () => {
  const { host, item, send } = await setup();
  const { job_id } = await send("before-import", "Proven conversation");
  await host.run(job_id);
  const prior = host.item(item).head;
  const creator = host.people.Alice!;
  assert.deepEqual(host.discoverable(creator), [item]);
  const body = Buffer.from(host.text(item) + "Imported material of unproven authorship\n" + "x".repeat(140 * 1024));
  // An importer outside any job, using chat.run's declared staging methods.
  const ctx = host.context("chat.run", { request_id: "import" });
  const target = { purpose: "import", item, expected: prior, body };
  const receipt = await saveBody(ctx, target);
  assert.deepEqual(await saveBody(ctx, target), receipt);
  await assert.rejects(saveBody(ctx, { ...target, body: body.subarray(1) }), /conflicting replay/);
  await assert.rejects(ctx.call("item_save", { request_id: ctx.key("import"), item, expected: prior, patch: { title: "Different save" } }), /conflicting replay/);
  assert.equal(host.text(item), body.toString());
  const stored = host.item(item);
  assert.equal(stored.discovery, "consent_only");
  assert.equal(stored.author, undefined);
  assert.deepEqual(stored.index.get(stored.head), { body_provenance: "unproven", spans: [] });
  assert.equal(stored.index.get(prior)?.spans.length, 3);
  assert.deepEqual(host.discoverable(creator), []);
  const revision = stored.head;
  await assert.rejects(ctx.call("item_save", { request_id: "try-clear-marker", item, expected: revision, attempt_discovery: "creator_only" as "consent_only" }),
    /does not match its schema/);
  assert.equal(stored.head, revision);
  assert.deepEqual(host.discoverable(creator), []);
});
