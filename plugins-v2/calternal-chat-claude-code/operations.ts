import { on, type ItemId, type RecordLink } from "@calternal/plugin-sdk";

/** The chat and newly appended prompt supplied by the Chat send operation. */
export interface StartInput {
  /** The session item fixed by the selected Chat send operation. */
  chat: ItemId;
  /** Host receipt link for the prompt record in that session. */
  prompt: RecordLink;
}

/** Starts the adapter in the open chat and supplies the new message in scope. */
export const start = on.call("runs.start", async ({ chat, prompt }: StartInput, s) => {
  const run = await s.run("calternal-provider-claude-code", { scope: "session", targets: [chat] });
  if (run.status !== "running") return { id: run.id, status: run.status };
  await run.send({
    version: 1,
    seq: 1,
    command: {
      type: "start",
      model: "claude-haiku-5-5",
      options: { effort: "low" },
      cwd: "/work",
      instructions: "Answer the supplied chat message.",
      resume: null,
    },
  });
  await run.send({
    version: 1,
    seq: 2,
    command: { type: "send", prompt, context: [] },
  });
  return { id: run.id, status: run.status };
});
