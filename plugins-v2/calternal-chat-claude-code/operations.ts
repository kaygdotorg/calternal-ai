import { on, type ItemId, type RecordLink } from "@calternal/plugin-sdk";

/** Inputs supplied by Chat for a session-scoped run. */
export interface StartInput {
  /** The only item the run may target. */
  chat: ItemId;
  /** Host-issued link to the record that prompted the run. */
  prompt: RecordLink;
}

/** Starts the adapter for the open chat and sends its prompting record link as the first input. */
export const start = on.call("runs.start", async ({ chat, prompt }: StartInput, s) => {
  const run = await s.run("calternal-provider-claude-code", { scope: "session", targets: [chat] });
  await run.send(prompt);
  return { id: run.id };
});
