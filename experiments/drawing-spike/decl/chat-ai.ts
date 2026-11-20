// calternal-chat-ai, copied from authoring.md section 2.3 unchanged: the spike needs its `call` declaration to derive the
// "Ask an agent" form from it.
// calternal-chat-ai: call an agent into a chat with the authors' consent.
import { plugin, link, text, list, operation, slot } from "./sdk.ts";
export default plugin({
  id: "calternal-chat-ai", name: "Chat agents", version: "2.0.0", icon: "spark",
  depends: [{ plugin: "calternal-chat", version: "^2" }],
  grant: { kinds: { chats: "read" }, calls: ["calternal-chat/send"] },
  slots: { harness: slot({ effect: "viewer", recommended: "calternal-chat-claude-code" }) },
  operations: {
    call: operation({ title: "Ask an agent", input: { chat: link(), range: list(text()), prompt: text(), harness: text() },
      async handler({ chat, range, prompt, harness }, ctx) {
        const { anchor: call } = await ctx.call<{ anchor: string }>("calternal-chat/send", { chat, text: prompt });
        // The host asks each other author, waits up to a minute, then freezes the input (permissions 1.6).
        const run = await ctx.work.start({
          request: ctx.request, program: `${harness}/agent`, place: "ask", key: `chat:${chat}`,
          reads: { records: range.map(anchor => ({ file: chat, anchor })), consent: "ask" },
          input: { chat, call, prompt },
        });
        return { run: run.id };
      } }),
    stop: operation({ title: "Stop the agent", input: { chat: link() },
      async handler({ chat }, ctx) {
        for (const row of await ctx.work.list({ key: `chat:${chat}`, state: "running" })) await ctx.work.stop(row.id, ctx.key(row.id));
        return {};
      } }),
    steer: operation({ title: "Tell the agent", input: { chat: link(), text: text() },
      async handler({ chat, text }, ctx) {
        const [row] = await ctx.work.list({ key: `chat:${chat}`, state: "running" });
        if (!row) throw new Error("No agent is working in this chat.");
        return ctx.work.send(row.id, { type: "input", text }, ctx.request);
      } }),
  },
});
