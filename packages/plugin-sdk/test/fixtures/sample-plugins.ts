// Two small packages for the fake host tests: a log app with an agent point
// and a harness that contributes to it. Written only with SDK authoring APIs.
import type { ItemSnapshotView } from "../../generated/author-types.js";
import {
  appendMessage, defineOperation, definePlugin, object, oneOfValues, publishView, readBody, ref, streamContribution, string,
} from "../../src/index.js";

const text = string({ minLength: 1, maxLength: 4096 });
const write = { required_capability: "content.write", risk: "reversible", idempotency: "request_id" } as const;
const execute = { required_capability: "plugins.use", risk: "external", idempotency: "request_id" } as const;
const read = { required_capability: "space.read", risk: "read", idempotency: "read_only" } as const;
const page = (body: string): { view: ItemSnapshotView } =>
  ({ view: { content: { type: "stack", children: [{ type: "snapshot-text", text: body.slice(-1000) || "Empty" }] } } });

export const create = defineOperation({
  ...write, id: "log.create", title: "New log", creates: "logs", methods: ["item_create"], write_origins: { host_input: "title" },
  input_schema: object({ title: string({ minLength: 1, maxLength: 120 }) }, "title"),
  output_schema: object({ created: ref("ItemRef") }, "created"),
  async handler({ title }, ctx) {
    const head = await ctx.call("item_create", {
      request_id: ctx.key("create"), kind: "logs", name: "Log.md", metadata: { title }, attested: { source: { kind: "input" } },
      attempt_discovery: "creator_only",
    });
    return { created: head.item };
  },
});

export const post = defineOperation({
  ...write, id: "log.post", title: "Post", target: "logs", write_origins: { host_input: "text" },
  methods: ["item_head", "item_append", "item_save", "item_save_begin", "item_save_chunk", "item_save_seal", "item_range", "item_spans", "item_view_context", "item_view_publish"],
  input_schema: object({ item: ref("ItemRef"), text, anchor: string({ pattern: "^[a-z0-9-]+$", maxLength: 64 }) }, "item", "text"),
  output_schema: object({ revision: ref("RevisionRef"), mode: oneOfValues("attested", "append", "inline", "staged") }, "revision", "mode"),
  async handler({ item, text, anchor }, ctx) {
    const head = await ctx.call("item_head", { item });
    const saved = await appendMessage(ctx, {
      purpose: "post", item, expected_revision: head.revision, prefix: "\n- ", text, suffix: "\n", source: { kind: "input" },
      ...(anchor && { entry_anchor: anchor }),
    });
    const body = await readBody(ctx, item, saved.revision);
    await publishView(ctx, { item, slot: "body", revision: saved.revision, render: () => page(body.toString()) });
    return { revision: saved.revision, mode: saved.mode };
  },
});

export const ask = defineOperation({
  ...execute, id: "log.ask", title: "Ask the agent", target: "logs", methods: ["start_job"], write_origins: { host_input: "text" },
  input_schema: object({ item: ref("ItemRef"), text, harness: ref("HarnessRef"), placement: ref("ChoiceRef") }, "item", "text", "harness", "placement"),
  output_schema: ref("ExecutionJobReceipt"),
  handler: ({ item, text, harness, placement }, ctx) =>
    ctx.call("start_job", { job: "answer", item, harness, placement, input: {}, contribution_input: { text } }),
});

export const remind = defineOperation({
  ...execute, id: "log.remind", title: "Ask later", target: "logs", methods: ["alarm_set"],
  input_schema: object({ item: ref("ItemRef"), text, harness: ref("HarnessRef"), at: ref("Timestamp") }, "item", "text", "harness", "at"),
  output_schema: ref("AlarmReceipt"),
  handler: ({ item, text, harness, at }, ctx) =>
    ctx.call("alarm_set", { request_id: ctx.key("alarm"), job: "answer", item, at, harness, input: {}, contribution_input: { text } }),
});

export const answer = defineOperation({
  ...execute, id: "log.answer", title: "Answer", target: "logs", write_origins: { attempt_output: "text" },
  methods: ["item_head", "item_append", "item_save", "item_save_begin", "item_save_chunk", "item_save_seal", "execution_start", "contribution_call", "contribution_read"],
  input_schema: object({ item: ref("ItemRef") }, "item"), output_schema: object({ replies: { type: "integer" } }, "replies"),
  async handler({ item }, ctx) {
    const job = ctx.job!;
    const { execution } = await ctx.call("execution_start", { request_id: ctx.key("start"), harness: job.harness });
    let replies = 0;
    for await (const frame of streamContribution(ctx, { purpose: "call", execution, contribution: job.harness, input: job.contribution_input })) {
      const head = await ctx.call("item_head", { item });
      await appendMessage(ctx, {
        purpose: `reply-${frame.seq}`, item, expected_revision: head.revision, prefix: "\n> agent: ", text: (frame.data as { text: string }).text,
        suffix: "\n", source: { kind: "output", seq: frame.seq },
      });
      replies++;
    }
    return { replies };
  },
});

export const state = defineOperation({
  ...write, required_capability: "space.read", id: "log.state", title: "Pin", target: "logs", methods: ["viewer_state_read", "viewer_state_write"],
  input_schema: object({ item: ref("ItemRef"), pinned: { type: "boolean" } }, "item", "pinned"), output_schema: ref("ViewerItemState"),
  async handler({ item, pinned }, ctx) {
    const prior = await ctx.call("viewer_state_read", { item });
    return ctx.call("viewer_state_write", { item, expected: prior.version, request_id: ctx.key("pin"), patch: { pinned } });
  },
});

export const refresh = defineOperation({
  ...read, id: "log.refresh", title: "Read log", target: "logs", methods: ["item_range"],
  input_schema: ref("ItemViewRead"), output_schema: ref("ItemViewPage"),
  handler: async (input, ctx) => page((await readBody(ctx, input.item, input.source_revision)).toString()),
});

const point = { text, output: object({ text }, "text") };

export const logApp = definePlugin({
  api_version: 2, id: "sample-log", name: "Log", version: "0.1.0", icon: "notes",
  kinds: [{
    id: "logs", icon: "notes", title_field: "title", storage: { folder: "Logs" }, claimed_text: true, append_file: true,
    attempt_discovery: "item_policy", execution_target: true, view_snapshots: [{ id: "body", refresh: "log.refresh" }],
    viewer_fields: { pinned: { type: "Boolean" }, unread: { type: "Number", source: "entry_unread" } },
    schema: object({ title: string() }, "title"),
  }],
  jobs: [{ id: "answer", run_operation: "log.answer", replayable: true, execution: true, alarm: true }],
  operations: [create, post, ask, remind, answer, state, refresh],
  apps: [{
    metadata: {
      id: "log", name: "Log", icon: "notes",
      extension_points: [{ id: "agents", version: 1, kind: "harness", input_schema: object({ text: point.text }, "text"), output_schema: point.output }],
    },
    template: { version: 2, kind: "app", revision: 1, binds: [] },
  }],
});

export const reply = defineOperation({
  ...execute, id: "echo.reply", title: "Echo", methods: [],
  input_schema: object({ text: point.text }, "text"), output_schema: point.output,
  handler: async ({ text }, ctx) => {
    const provider = (ctx as unknown as { provider?: (text: string) => Promise<string> }).provider;
    return { text: provider ? await provider(text) : `echo: ${text}` };
  },
});

export const echoHarness = definePlugin({
  api_version: 2, id: "sample-echo", name: "Echo", version: "0.1.0", icon: "puzzle", kinds: [],
  contributions: [{
    app: { package: "sample-log", app: "log" }, point: "agents", version: 1, id: "echo", kind: "harness", operation: "echo.reply",
    harness: { image: `sha256:${"0".repeat(64)}`, image_source: "registry.example.invalid/echo", execution_network: "none", entry: { path: "entry.bin", sha256: "0".repeat(64) }, name: "Echo" },
  }],
  operations: [reply],
  apps: [],
});
