// Compile-time checks for schema-derived operation types. Run with `npm run test:types`.
import { defineOperation, handlersOf, object, oneOfValues, ref, string, type OperationContext } from "../../src/index.js";

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends (<T>() => T extends B ? 1 : 2) ? true : false;
const assert = <T extends true>() => undefined as unknown as T;

const send = defineOperation({
  id: "chat.send", title: "Send", required_capability: "plugins.use", risk: "external", idempotency: "request_id",
  target: "chats", methods: ["item_head", "start_job"],
  input_schema: object({ item: ref("ItemRef"), text: string({ maxLength: 16 }), mode: oneOfValues("run", "skip") }, "item", "text"),
  output_schema: ref("ExecutionJobReceipt"),
  async handler(input, ctx) {
    assert<Equal<typeof input, { item: string; text: string; mode?: "run" | "skip" }>>();
    const head = await ctx.call("item_head", { item: input.item });
    assert<Equal<typeof head.revision, string>>();
    // @ts-expect-error item_append is not declared in methods.
    await ctx.call("item_append", { request_id: ctx.key("x"), item: input.item, expected_revision: head.revision, bytes: "" });
    // @ts-expect-error item_head needs an item.
    await ctx.call("item_head", {});
    return ctx.call("start_job", { job: "run", item: input.item, input: {}, harness: "h", contribution_input: { text: input.text } });
  },
});
assert<Equal<typeof send.target, { kind: string; field: string } | { kind: string; selection_field: string } | undefined>>();

defineOperation({
  id: "chat.bad", title: "Bad", required_capability: "space.read", risk: "read", idempotency: "read_only", methods: [],
  input_schema: object({ title: string() }, "title"), output_schema: object({ ok: { const: true } }, "ok"),
  // @ts-expect-error the result must match output_schema.
  handler: async () => ({ ok: false }),
});

const handlers = handlersOf([send]);
void handlers;
const helper = (ctx: OperationContext<"item_head">) => ctx;
// A context with more declared methods satisfies a helper that needs fewer.
void ((ctx: OperationContext<"item_head" | "start_job">) => helper(ctx));
// @ts-expect-error a context without item_head cannot call a helper that needs it.
void ((ctx: OperationContext<"start_job">) => helper(ctx));
