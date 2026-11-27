import { on, type ItemId } from "@calternal/plugin-sdk";

/** The open chat supplied by the harness place for a session-scoped run. */
export interface StartInput {
  chat: ItemId;
}

/** Starts the adapter with the open chat as its sole target. */
export const start = on.call("runs.start", async ({ chat }: StartInput, s) => {
  const run = await s.run("calternal-provider-claude-code", { scope: "session", targets: [chat] });
  return { id: run.id };
});
