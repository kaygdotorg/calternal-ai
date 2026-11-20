import { createHash } from "node:crypto";
import { encodeContributionFrame } from "../../calternal-threads/generated/contribution-frame.mjs";
import { checkedText, type HarnessInput, type HarnessOutput } from "@calternal/threads/harnesses-v1";
import type { Context } from "../../calternal-threads/src/interface.ts";

type HarnessContext = Pick<Context, "call" | "author"> & {
  // The fake supplies provider output. Live placement runs the pinned entry.
  claude: (text: string) => Promise<string>;
};

async function run({ text }: HarnessInput, ctx: HarnessContext): Promise<HarnessOutput> {
  const prompt = checkedText(text);
  if (!ctx.claude) throw new Error("This host does not expose the harness entry ABI.");
  const key = (purpose: string) =>
    createHash("sha256").update(JSON.stringify([ctx.author.request_id, purpose])).digest("hex").slice(0, 32);
  const id = key("log");
  const title = "Claude Code run";
  const log = `---\nid: ${id}\ntitle: ${title}\n---\n\n## Prompt\n\n${prompt}\n`;
  const saved = await ctx.call("item_create", {
    kind: "claude-code-run", name: "Claude Code run.md", request_id: id,
    metadata: { title }, body: Buffer.from(log).toString("base64"),
  });
  const reply = checkedText(await ctx.claude(prompt));
  const output = { text: reply, log: saved.locator };
  encodeContributionFrame({ seq: 1, kind: "output", data: output });
  await ctx.call("item_append", {
    item: saved.item, expected_revision: saved.revision, request_id: key("reply"),
    bytes: Buffer.from(`\n## Reply\n\n${reply}\n`).toString("base64"),
  });
  return output;
}

/** The contributing worker used by the fake placement when it runs the entry. */
export const handlers = { "threads-claude-code.run": run };
