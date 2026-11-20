import { definePlugin, object, string } from "@calternal/plugin-sdk";
import { description } from "./description.ts";
import { harnessInput, harnessOutput } from "./harnesses-v1.ts";
import { viewerFields } from "./viewer.ts";
import { operations } from "./worker.ts";

const title = string({ minLength: 1, maxLength: 120 });

export default definePlugin({
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
    schema: object({ title, shape: { enum: ["person", "group", "agent", "mixed"] } }, "title"),
    indexed_fields: { shape: { type: "Text", audience: "shared_projection" } },
    viewer_fields: viewerFields,
  }],
  jobs: [{ id: "run", run_operation: "chat.run", replayable: true, execution: true, alarm: true }],
  operations,
  apps: [{
    metadata: {
      id: "chat", name: "Chat", icon: "chat",
      extension_points: [{ id: "harnesses", version: 1, kind: "harness", input_schema: harnessInput, output_schema: harnessOutput }],
    },
    template: description,
  }],
});
