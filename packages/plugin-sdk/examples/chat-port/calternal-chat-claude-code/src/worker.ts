import { defineOperation, encodeContributionFrame, handlersOf } from "@calternal/plugin-sdk";
import { checkedText, harnessInput, harnessOutput } from "../../calternal-chat/src/harnesses-v1.ts";

// The fake supplies provider output through `claude`. Live placement runs the pinned entry.
type Provider = { claude?: (text: string) => Promise<string> };

const run = defineOperation({
  id: "chat-claude-code.run", title: "Run Claude Code", required_capability: "plugins.use", risk: "external", idempotency: "request_id",
  methods: ["item_create", "item_append"], input_schema: harnessInput, output_schema: harnessOutput,
  async handler({ text }, ctx) {
    const prompt = checkedText(text);
    const { claude } = ctx as typeof ctx & Provider;
    if (!claude) throw new Error("The Claude Code plugin cannot run on this host.");
    const title = "Claude Code run";
    const log = `---\nid: ${ctx.key("log")}\ntitle: ${title}\n---\n\n## Prompt\n\n${prompt}\n`;
    const saved = await ctx.call("item_create", {
      kind: "claude-code-run", name: "Claude Code run.md", request_id: ctx.key("log"),
      metadata: { title }, body: Buffer.from(log).toString("base64"),
    });
    const reply = checkedText(await claude(prompt));
    const output = { text: reply, log: saved.locator };
    encodeContributionFrame({ seq: 1, kind: "output", data: output });
    await ctx.call("item_append", {
      item: saved.item, expected_revision: saved.revision, request_id: ctx.key("reply"),
      bytes: Buffer.from(`\n## Reply\n\n${reply}\n`).toString("base64"),
    });
    return output;
  },
});

export const operations = [run];

/** The contributing worker used by the fake placement when it runs the entry. */
export const handlers = handlersOf(operations);
