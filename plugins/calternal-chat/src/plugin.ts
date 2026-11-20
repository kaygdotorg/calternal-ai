import { definePlugin } from "@calternal/plugin-sdk";
import { description } from "./description.ts";
import type { ManifestV2 } from "../generated/author.js";
import { viewerFields, viewerPatchSchema } from "./viewer.ts";
import { catalogueRef, harnessInput, harnessOutput, object, textSchema } from "./harnesses-v1.ts";

const item = catalogueRef("ItemRef");
const harness = catalogueRef("HarnessRef");
const placement = catalogueRef("ChoiceRef");
const title = { type: "string", minLength: 1, maxLength: 120 };
const writeView = ["item_view_context", "item_view_publish"] as const;
const readBody = ["item_head", "item_range", "item_spans"] as const;
const target = { kind: "chats", field: "item" };
const write = { required_capability: "content.write", risk: "reversible", idempotency: "request_id" } as const;
const read = { required_capability: "space.read", risk: "read", idempotency: "read_only" } as const;
const execute = { required_capability: "plugins.use", risk: "external", idempotency: "request_id" } as const;
const empty = object({});
const ifMissed = { enum: ["run", "skip"] };
const messageResult = {
  id: { type: "string" },
  author: object({ kind: { enum: ["person", "agent"] }, id: { type: "string" }, name: { type: "string" }, contribution: catalogueRef("ContributionIdentity") }, "kind", "id", "name"),
};
const messageIdPattern = "^(?:[0-9a-f]{16}|[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12})$";
const messageOffset = { type: "integer", minimum: 0, maximum: 2 * 1024 * 1024 };
const legacyMessageResult = object({
  id: { type: "string" }, role: { enum: ["person", "agent", "automation"] }, heading: { type: "string" },
  text: { type: "string", maxLength: 8000 }, posted_at: catalogueRef("Timestamp"), turn: { type: "string" },
  next_offset: messageOffset,
}, "id", "role", "heading", "text", "posted_at");
const app = {
  id: "chat", name: "Chat", icon: "chat",
  extension_points: [{ id: "harnesses", version: 1, kind: "harness", input_schema: harnessInput, output_schema: harnessOutput }],
} satisfies Omit<ManifestV2["apps"][number], "description">;
const metadata = {
  api_version: 2, id: "calternal-chat", name: "Chat", version: "0.1.0", icon: "chat",
  kinds: [{
    id: "chat-consultations", icon: "chat", title_field: "title", storage: { hidden: true },
    claimed_text: true, schema: object({ title }, "title"),
  }, {
    id: "chats", icon: "chat", title_field: "title", storage: { folder: "Chats" },
    claimed_text: true, searchable: true, file_format: {
      id: "calternal-chat", version: 2, default_app: "chat",
      title_binding: { placement: "utc_date_folder", frontmatter_key: "title" },
      metadata_tokens: [
        { scope: "frontmatter_key", prefix: "format" },
        { scope: "frontmatter_key", prefix: "id" },
        { scope: "frontmatter_line", prefix: "participants:" },
        { scope: "frontmatter_line", prefix: "  - p" },
        { scope: "frontmatter_key", prefix: "agent-p", suffix: "-plugin" },
        { scope: "frontmatter_key", prefix: "agent-p", suffix: "-harness" },
        { scope: "framing_suffix", prefix: " {#m-", suffix: "}" },
        { scope: "framing_line", prefix: "<!-- run-log=", suffix: " -->" },
        { scope: "framing_line", prefix: "<!-- consultation=", suffix: " -->" },
        { scope: "framing_line", prefix: "<!-- reply-in=", suffix: " -->" },
      ],
    },
    append_file: true, attempt_discovery: "item_policy", execution_target: true, view_snapshots: [{ id: "body", refresh: "chat.refresh" }],
    schema: object({ title, shape: { type: "string", enum: ["person", "group", "agent", "mixed"] } }, "title"),
    indexed_fields: { shape: { type: "Text", audience: "shared_projection" } },
    viewer_fields: viewerFields,
  }],
  jobs: [{ id: "run", run_operation: "chat.run", replayable: true, execution: true, alarm: true }],
  operations: [
    { ...write, id: "chat.create", title: "New chat", creates: "chats", methods: ["item_create", ...writeView],
      write_origins: { host_input: "title" },
      input_schema: object({ title }, "title"), output_schema: object({ created: item }, "created") },
    { ...execute, id: "chat.send", title: "Send", target, methods: [...readBody, "start_job"], material_inputs: { placement: "Machine and login" },
      write_origins: { host_input: "text" },
      input_schema: object({ item, text: textSchema, harness, placement }, "item", "text", "harness", "placement"),
      output_schema: catalogueRef("ExecutionJobReceipt") },
    { ...execute, id: "chat.run", title: "Run agent", target, methods: [...readBody, "item_append", "item_save",
      "item_save_begin", "item_save_chunk", "item_save_seal", "execution_start", "contribution_call", "contribution_read", ...writeView],
      write_origins: { attempt_output: "text" },
      input_schema: object({ item, if_missed: ifMissed }, "item"), output_schema: empty },
    { ...execute, id: "chat.arm", title: "Schedule a run", target, methods: [...readBody, "alarm_set"],
      input_schema: object({ item, text: textSchema, harness, at: catalogueRef("Timestamp"), if_missed: ifMissed, grace_seconds: { type: "integer", minimum: 0, maximum: 86400 } },
        "item", "text", "harness", "at", "if_missed"), output_schema: catalogueRef("AlarmReceipt") },
    { ...write, required_capability: "space.read", id: "chat.state", title: "Update my thread preferences", target,
      methods: ["viewer_state_read", "viewer_state_write"],
      input_schema: object({ item, ...viewerPatchSchema }, "item"), output_schema: catalogueRef("ViewerItemState") },
    { ...read, id: "chat.refresh", title: "Read thread page", target, methods: [...readBody],
      input_schema: catalogueRef("ItemViewRead"), output_schema: catalogueRef("ItemViewPage") },
    { ...read, id: "chat.message", title: "Read message", target, methods: [...readBody],
      input_schema: object({ item, message: { type: "string", pattern: messageIdPattern }, offset: messageOffset }, "item", "message"),
      output_schema: { oneOf: [object({ ...messageResult, text: textSchema, posted_at: catalogueRef("Timestamp"),
        log: catalogueRef("ItemLocator"), consultation: catalogueRef("ItemLocator"), edited: { const: true }, replyTo: { type: "string" }, replyToName: { type: "string" }, replyIn: catalogueRef("ItemLocator"), replyQuote: textSchema,
        askedBy: object({ kind: { const: "person" }, id: { type: "string" }, name: { type: "string" } }, "kind", "id", "name"),
        mentions: { type: "array", items: catalogueRef("AccountRef") } }, "id", "author", "text", "posted_at"), legacyMessageResult] } },
  ],
} satisfies Omit<ManifestV2, "apps" | "worker" | "network">;

// The rev13 SDK author builder has older declaration types. Its runtime only
// supplies defaults and packaging metadata. Check the draft shape above.
const authored = definePlugin({ ...metadata, apps: [] } as unknown as Parameters<typeof definePlugin>[0]);
export default { ...authored, apps: [{ metadata: app, template: description }] };
