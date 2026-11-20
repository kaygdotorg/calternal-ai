import { describe, expect, test } from "vitest";
import {
  checkItemView, defineOperation, encodeContributionFrame, formField, LimitError, LIMITS, object, readBody, readSpans, ref, saveBody,
} from "../src/index.js";
import { FakeHost, HostRefusal } from "../src/testing.js";
import { create, echoHarness, logApp, post } from "./fixtures/sample-plugins.js";

const { ITEM_INLINE_BODY_BYTES } = LIMITS;

async function setup() {
  const host = new FakeHost([logApp, echoHarness]);
  const { created: item } = await host.invoke("log.create", { title: "Field notes" }, { captured: true }) as { created: string };
  return { host, item, choice: host.choose() };
}

describe("defineOperation", () => {
  test("infers the target field and keeps the manifest entry explicit", () => {
    expect(post.target).toEqual({ kind: "logs", field: "item" });
    expect(JSON.parse(JSON.stringify(post))).toEqual({
      required_capability: "content.write", risk: "reversible", idempotency: "request_id", id: "log.post", title: "Post",
      write_origins: { host_input: "text" }, methods: post.methods, input_schema: post.input_schema, output_schema: post.output_schema,
      target: { kind: "logs", field: "item" },
    });
    expect(() => defineOperation({
      id: "x.y", title: "Y", required_capability: "space.read", risk: "read", idempotency: "read_only", methods: [], target: "logs",
      input_schema: object({ a: ref("ItemRef"), b: ref("ItemRef") }, "a"), output_schema: true,
    })).toThrow(/exactly one ItemRef/);
  });

  test("derives a description form field from the input schema", () => {
    expect(formField(create, "title", { label: "Title", placeholder: "What is it about?" }))
      .toEqual({ id: "title", input: "text", required: true, maxLength: 120, label: "Title", placeholder: "What is it about?" });
  });
});

describe("fake host", () => {
  test("creates, appends attested input, records spans and replays receipts", async () => {
    const { host, item } = await setup();
    expect(host.item(item).path).toBe("Logs/Log.md");
    const result = await host.invoke("log.post", { item, text: "First 🦊", anchor: "a1" }, { captured: true, request_id: "post-1" });
    expect(result).toMatchObject({ mode: "attested" });
    expect(host.text(item)).toBe("Field notes\n- First 🦊\n");
    // A replay that re-reads the moved head changes the append's input under the same key.
    await expect(host.invoke("log.post", { item, text: "First 🦊", anchor: "a1" }, { captured: true, request_id: "post-1" }))
      .rejects.toThrow(/conflicting replay/);
    const ctx = host.context("log.post");
    const { spans, body_provenance } = await readSpans(ctx, item, host.item(item).head, 64);
    expect(body_provenance).toBe("attested");
    expect(spans.map(span => span.attribution)).toEqual([{ kind: "person", actor: host.people.Alice }, { kind: "person", actor: host.people.Alice }]);
    expect(host.discoverable(host.people.Alice!)).toEqual([item]);
    expect(host.view(item, "body")?.source_revision).toBe(host.item(item).head);
  });

  test("refuses an undeclared method with the manifest path and the change to make", async () => {
    const { host, item } = await setup();
    const ctx = host.context<string>("log.ask") as unknown as { call(method: string, input: unknown): Promise<unknown> };
    const refusal = await ctx.call("item_head", { item }).catch(error => error) as HostRefusal;
    expect(refusal).toBeInstanceOf(HostRefusal);
    expect(refusal).toMatchObject({ status: 403, code: "not_declared", path: "operations[2].methods" });
    expect(refusal.message).toMatch(/Add "item_head" to the methods of log.ask/);
  });

  test("schema-checks broker inputs and operation inputs", async () => {
    const { host, item } = await setup();
    await expect(host.context("log.post").call("item_head", { item, extra: 1 } as never)).rejects.toMatchObject({ code: "invalid_shape" });
    await expect(host.invoke("log.post", { item, text: "" })).rejects.toMatchObject({ code: "invalid_shape" });
    await expect(host.invoke("log.post", { item: "nope", text: "x" })).rejects.toThrow(/target unavailable/);
  });

  test("an input-attested write needs a captured submission; an opaque one downgrades discovery", async () => {
    const { host, item } = await setup();
    const ctx = host.context("log.post");
    const head = await ctx.call("item_head", { item });
    await expect(ctx.call("item_append", { request_id: "r", item, expected_revision: head.revision, attested: { source: { kind: "input" } } }))
      .rejects.toMatchObject({ code: "missing_capture" });
    await ctx.call("item_append", { request_id: "r2", item, expected_revision: head.revision, bytes: Buffer.from("raw").toString("base64") });
    expect(host.item(item).provenance).toBe("unproven");
    expect(host.discoverable(host.people.Alice!)).toEqual([]);
  });

  test("saveBody stages a large body and readBody reads it back across ranges", async () => {
    const { host, item } = await setup();
    const ctx = host.context("log.post");
    const body = Buffer.alloc(ITEM_INLINE_BODY_BYTES * 2 + 17, "x");
    const saved = await saveBody(ctx, { purpose: "big", item, expected: host.item(item).head, body });
    expect("revision" in saved).toBe(true);
    expect(host.calls.filter(call => call.method === "item_save_chunk")).toHaveLength(5);
    expect((await readBody(ctx, item, host.item(item).head)).equals(body)).toBe(true);
    const conflict = await saveBody(ctx, { purpose: "stale", item, expected: "old-revision", body: Buffer.from("small") });
    expect(conflict).toMatchObject({ status: 409, code: "head_conflict" });
  });

  test("span pages join across cursors", async () => {
    const { host, item } = await setup();
    for (const [index, text] of ["one", "two", "three"].entries()) {
      await host.invoke("log.post", { item, text }, { captured: true, request_id: `p${index}` });
    }
    host.items.spanPageSize = 1;
    const { spans } = await readSpans(host.context("log.post"), item, host.item(item).head, 1000);
    expect(spans).toHaveLength(4);
    expect(host.calls.filter(call => call.method === "item_spans")).toHaveLength(4);
  });

  test("viewer state is compare-and-set, per person, with host-maintained unread counts", async () => {
    const { host, item } = await setup();
    await host.invoke("log.post", { item, text: "hello", anchor: "e1" }, { captured: true, request_id: "e1" });
    const bob = host.people.Bob!;
    expect(await host.invoke("log.state", { item, pinned: true }, { actor: bob })).toMatchObject({ fields: { pinned: true, unread: 1 } });
    host.items.markRead(item, bob, "e1");
    expect(await host.invoke("log.state", { item, pinned: false }, { actor: bob })).toMatchObject({ fields: { pinned: false, unread: 0 } });
    expect(await host.invoke("log.state", { item, pinned: true })).toMatchObject({ fields: { pinned: true, unread: 0 } });
    const ctx = host.context("log.state", { actor: bob });
    await expect(ctx.call("viewer_state_write", { item, expected: "initial", request_id: "stale", patch: { pinned: true } }))
      .rejects.toMatchObject({ code: "version_conflict" });
    await expect(ctx.call("viewer_state_write", { item, expected: (await ctx.call("viewer_state_read", { item })).version, request_id: "u", patch: { unread: 3 } }))
      .rejects.toMatchObject({ code: "field_not_writable" });
  });

  test("a job runs a contribution and writes output-attested replies", async () => {
    const { host, item, choice } = await setup();
    const { job_id } = await host.invoke("log.ask", { item, text: "Hi", ...choice }, { captured: true }) as { job_id: string };
    expect(await host.run(job_id)).toEqual({ replies: 1 });
    expect(host.text(item)).toBe("Field notes\n> agent: echo: Hi\n");
    const spans = host.item(item).spans;
    expect(spans.at(-1)?.attribution).toEqual({ kind: "attempt", attempt: host.jobs.get(job_id)!.attempt!.execution });
  });

  test("the queue holds one successor, Stop fences the attempt and teardown gates the next", async () => {
    const { host, item, choice } = await setup();
    let release!: () => void;
    const gate = new Promise<void>(resolve => { release = resolve; });
    let reached!: () => void;
    const started = new Promise<void>(resolve => { reached = resolve; });
    host.provide("sample-echo", { provider: async (text: string) => { reached(); await gate; return `late ${text}`; } });
    host.execution.readWaitMs = 5;
    const first = await host.invoke("log.ask", { item, text: "one", ...choice }, { request_id: "a" }) as { job_id: string };
    const second = await host.invoke("log.ask", { item, text: "two", ...choice }, { request_id: "b" }) as { job_id: string };
    await expect(host.invoke("log.ask", { item, text: "three", ...choice }, { request_id: "c" })).rejects.toThrow(/pending_limit/);
    await expect(host.run(second.job_id)).rejects.toThrow(/successor must wait/);
    const run = host.run(first.job_id);
    await started;
    host.stop(item, { execution: host.jobs.get(first.job_id)!.attempt!.execution });
    expect(() => host.stop(item, { execution: host.jobs.get(first.job_id)!.attempt!.execution })).toThrow(/stale Stop/);
    await expect(host.run(second.job_id)).rejects.toThrow(/teardown/);
    release();
    await expect(run).rejects.toThrow(/attempt retired/);
    expect(host.text(item)).not.toMatch(/late/);
    host.settle(first.job_id);
    await host.run(second.job_id);
    expect(host.text(item)).toMatch(/late two/);
  });

  test("admission refuses unchecked choices and oversized contribution input before any write", async () => {
    const { host, item, choice } = await setup();
    await expect(host.invoke("log.ask", { item, text: "x", ...choice, harness: "forged" })).rejects.toThrow(/unchecked harness/);
    await expect(host.invoke("log.ask", { item, text: "x", ...choice, placement: "forged" })).rejects.toThrow(/unchecked placement/);
    await expect(host.invoke("log.ask", { item, text: "\u0001".repeat(3000), ...choice })).rejects.toMatchObject({ status: 413, code: "contribution_frame_limit" });
    const fresh = host.choose();
    await expect(host.invoke("log.ask", { item, text: "x", ...choice })).rejects.toThrow(/unchecked harness/);
    await host.invoke("log.ask", { item, text: "x", ...fresh });
    expect(host.jobs.size).toBe(1);
  });

  test("alarms admit one job at delivery with missed evidence", async () => {
    const { host, item, choice } = await setup();
    const { alarm } = await host.invoke("log.remind", { item, text: "later", harness: choice.harness, at: "2026-11-20T13:00:00Z" }) as { alarm: string };
    expect(host.execution.alarms.get(alarm)?.set.item).toBe(item);
    await host.deliver(alarm, "2026-11-20T13:02:00Z");
    const [job] = host.jobs.values();
    expect(job!.alarm).toMatchObject({ alarm, missed: true });
    expect(await host.deliver(alarm, "2026-11-20T13:02:00Z")).toBeUndefined();
    expect(host.text(item)).toMatch(/echo: later/);
  });

  test("a lost acknowledgement commits once and the same call replays the receipt", async () => {
    const { host, item } = await setup();
    const ctx = host.context("log.post");
    const input = { request_id: "lost", item, expected_revision: host.item(item).head, bytes: Buffer.from(" once").toString("base64") };
    host.failNext("item_append", "lose_ack");
    await expect(ctx.call("item_append", input)).rejects.toThrow(/acknowledgement lost/);
    const receipt = await ctx.call("item_append", input);
    expect(receipt.revision).toBe(host.item(item).head);
    expect(host.text(item).match(/once/g)).toHaveLength(1);
  });
});

describe("limits", () => {
  test("preflights name the limit, the bound and the value", () => {
    expect(() => encodeContributionFrame({ text: "x".repeat(20000) })).toThrow(LimitError);
    expect(() => encodeContributionFrame({ text: "x".repeat(20000) }))
      .toThrow(/CONTRIBUTION_FRAME_BYTES allows 16384 \(catalogue#\/extension_points\/calls\/limits\/encoded_frame_bytes\)/);
    expect(() => checkItemView({ content: { type: "snapshot-text", text: "é".repeat(1500) } })).toThrow(/SNAPSHOT_TEXT_CHARACTERS/);
  });
});
