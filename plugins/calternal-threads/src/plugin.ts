import { definePlugin } from "@calternal/plugin-sdk";
import type { AppDescription } from "../generated/wire.js";
import type { ManifestV2 } from "../generated/author.js";
import { catalogueRef, harnessInput, harnessOutput, object, textSchema } from "./harnesses-v1.ts";

const item = catalogueRef("ItemRef");
const harness = catalogueRef("HarnessRef");
const placement = catalogueRef("ChoiceRef");
const title = { type: "string", minLength: 1, maxLength: 120 };
const writeView = ["item_view_context", "item_view_publish"] as const;
const readBody = ["item_head", "item_range"] as const;
const target = { kind: "threads", field: "item" };
const write = { required_capability: "content.write", risk: "reversible", idempotency: "request_id" } as const;
const read = { required_capability: "space.read", risk: "read", idempotency: "read_only" } as const;
const execute = { required_capability: "plugins.use", risk: "external", idempotency: "request_id" } as const;
const empty = object({});
const ifMissed = { enum: ["run", "skip"] };
const messageResult = {
  id: { type: "string" },
  author: object({ kind: { enum: ["person", "agent"] }, id: { type: "string" } }, "kind", "id"),
};
const itemRoute = { route: "item" };
const harnessChoice = { prompt: { title: "Harness", kind: "choice", options: { extension: "harnesses" } } } as const;
const description: AppDescription = {
  version: 2, kind: "app", revision: 1,
  view: { root: { type: "stack", children: [
    { type: "form", id: "create", fields: [{ id: "title", label: "Title", input: "text", required: true }],
      submit: { id: "create-thread", title: "New thread", operation: "threads.create",
        inputs: { title: { form: "create", field: "title" } }, then: { route: { name: "item" } } } },
    { type: "component", component: { type: "item-view", id: "transcript", config: { item: itemRoute, slot: "body" } } },
    { type: "form", id: "composer", fields: [{ id: "message", label: "Message", input: "multiline", required: true, maxLength: 4000 }],
      submit: { id: "send", title: "Send", operation: "threads.send", inputs: {
        item: itemRoute, text: { form: "composer", field: "message" }, harness: harnessChoice,
        placement: { prompt: { title: "Run on", kind: "choice", options: { host: "execution_placement" } } },
      } } },
    { type: "form", id: "rename", fields: [{ id: "title", label: "Title", input: "text", required: true }],
      submit: { id: "rename-thread", title: "Rename", operation: "threads.rename",
        inputs: { item: itemRoute, title: { form: "rename", field: "title" } } } },
    { type: "button", invoke: { id: "archive-thread", title: "Archive", operation: "host.item.archive", inputs: { item: itemRoute } } },
  ] } },
};
const app = {
  id: "threads", name: "Threads", icon: "threads",
  extension_points: [{ id: "harnesses", version: 1, kind: "harness", input_schema: harnessInput, output_schema: harnessOutput }],
} satisfies Omit<ManifestV2["apps"][number], "description">;
const metadata = {
  api_version: 2, id: "calternal-threads", name: "Threads", version: "0.1.0", icon: "threads",
  kinds: [{
    id: "threads", icon: "threads", title_field: "title", storage: { folder: "Threads" },
    claimed_text: true, searchable: true, file_format: { id: "calternal-thread", version: 1 },
    append_file: true, execution_target: true, view_snapshots: [{ id: "body", refresh: "threads.refresh" }],
    schema: object({ title }, "title"),
  }],
  jobs: [{ id: "run", run_operation: "threads.run", replayable: true, execution: true, alarm: true }],
  operations: [
    { ...write, id: "threads.create", title: "New thread", creates: "threads", methods: ["item_save", ...writeView],
      input_schema: object({ title }, "title"), output_schema: object({ created: item }, "created") },
    { ...execute, id: "threads.send", title: "Send", target, methods: [...readBody, "start_job"], material_inputs: { placement: "Machine and login" },
      input_schema: object({ item, text: textSchema, harness, placement }, "item", "text", "harness", "placement"),
      output_schema: catalogueRef("ExecutionJobReceipt") },
    { ...write, id: "threads.rename", title: "Rename", target, methods: [...readBody, "item_save", ...writeView],
      input_schema: object({ item, title }, "item", "title"), output_schema: empty },
    { ...execute, id: "threads.run", title: "Run harness", target, methods: [...readBody, "item_save", "item_append", "execution_start", ...writeView],
      input_schema: object({ item, text: textSchema, if_missed: ifMissed }, "item", "text"), output_schema: empty },
    { ...execute, id: "threads.arm", title: "Schedule a run", target, methods: [...readBody, "alarm_set"],
      input_schema: object({ item, text: textSchema, harness, at: catalogueRef("Timestamp"), if_missed: ifMissed },
        "item", "text", "harness", "at", "if_missed"), output_schema: catalogueRef("AlarmReceipt") },
    { ...read, id: "threads.refresh", title: "Read thread page", target, methods: ["item_range"],
      input_schema: catalogueRef("ItemViewRead"), output_schema: catalogueRef("ItemViewPage") },
    { ...read, id: "threads.message", title: "Read message", target, methods: [...readBody],
      input_schema: object({ item, message: { type: "string", pattern: "^[0-9a-f]{32}$" } }, "item", "message"),
      output_schema: { oneOf: [
        object({ ...messageResult, text: textSchema, log: item, edited: { const: true } }, "id", "author", "text"),
        object({ ...messageResult, deleted: { const: true } }, "id", "author", "deleted"),
      ] } },
  ],
} satisfies Omit<ManifestV2, "apps" | "worker" | "network">;

// The rev13 SDK author builder has older declaration types. Its runtime only
// supplies defaults and packaging metadata. Check the draft shape above.
const authored = definePlugin({ ...metadata, apps: [] } as unknown as Parameters<typeof definePlugin>[0]);
export default { ...authored, apps: [{ metadata: app, template: description }] };
