import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { definePlugin } from "@calternal/plugin-sdk";
import type { ManifestV2 } from "../../calternal-chat/generated/author.js";
import { harnessInput, harnessOutput, object } from "@calternal/chat/harnesses-v1";

const entry = new URL("./fixture-entry.mjs", import.meta.url);
const sha256 = createHash("sha256").update(readFileSync(entry)).digest("hex");

// Schema-valid descriptor for the offline spike. This digest identifies only
// the fixture image marker. There is no approved or runnable OCI image here.
const metadata = {
  api_version: 2, id: "calternal-chat-claude-code", name: "Claude Code", version: "0.1.0", icon: "puzzle",
  kinds: [{
    id: "claude-code-run", icon: "puzzle", title_field: "title", storage: { hidden: true }, claimed_text: true, append_file: true,
    schema: object({ title: { type: "string" } }, "title"),
  }],
  contributions: [{
    app: { package: "calternal-chat", app: "chat" }, point: "harnesses", version: 1, id: "claude-code", kind: "harness",
    operation: "chat-claude-code.run",
    harness: {
      image: `sha256:${sha256}`,
      image_source: "registry.example.invalid/calternal/chat-claude-code-fixture",
      execution_network: "none",
      entry: { path: "fixture-entry.mjs", sha256 },
      name: "Claude Code", login_directories: [".claude"],
    },
  }],
  operations: [{
    id: "chat-claude-code.run", title: "Run Claude Code",
    required_capability: "plugins.use", risk: "external", idempotency: "request_id",
    methods: ["item_create", "item_append"], input_schema: harnessInput, output_schema: harnessOutput,
  }],
} satisfies Omit<ManifestV2, "apps" | "worker" | "network">;
const authored = definePlugin({ ...metadata, apps: [] } as unknown as Parameters<typeof definePlugin>[0]);
export default { ...authored, metadata: { ...authored.metadata, contributions: metadata.contributions } };
