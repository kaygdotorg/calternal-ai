import { createHash } from "node:crypto";
import { checkedText, type HarnessInput, type HarnessOutput } from "@calternal/threads/harnesses-v1";
import type { ItemAppend, ItemAppendReceipt, RevisionRef } from "../../calternal-threads/generated/wire.js";

type LogCreate = {
  kind: "claude-code-run";
  request_id: string;
  metadata: { title: string };
  bytes: string;
};
type HarnessContext = {
  request: string;
  call: ((method: "item_save", input: LogCreate) => Promise<{ item: HarnessOutput["log"]; revision: RevisionRef }>)
    & ((method: "item_append", input: ItemAppend) => Promise<ItemAppendReceipt>);
  // Test adapter for the unpinned harness entry ABI. No provider logins or
  // process spawning belong in the plugin worker.
  claude: (text: string) => Promise<string>;
};

async function run({ text }: HarnessInput, ctx: HarnessContext): Promise<HarnessOutput> {
  const prompt = checkedText(text);
  if (!ctx.claude) throw new Error("This host does not expose the harness entry ABI.");
  const key = (purpose: string) =>
    createHash("sha256").update(JSON.stringify([ctx.request, purpose])).digest("hex").slice(0, 32);
  const id = key("log");
  const title = "Claude Code run";
  const log = `---\nid: ${id}\ntitle: ${title}\n---\n\n## Prompt\n\n${prompt}\n`;
  const saved = await ctx.call("item_save", {
    kind: "claude-code-run", request_id: id, metadata: { title }, bytes: Buffer.from(log).toString("base64"),
  });
  const reply = checkedText(await ctx.claude(prompt));
  await ctx.call("item_append", {
    item: saved.item, expected_revision: saved.revision, request_id: key("reply"),
    bytes: Buffer.from(`\n## Reply\n\n${reply}\n`).toString("base64"),
  });
  return { reply, log: saved.item };
}

/** The contributed harness signature. The host supplies the exact attempt's entry adapter. */
export const handlers = { "threads-claude-code.run": run };
