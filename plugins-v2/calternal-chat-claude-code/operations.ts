import { on, type ItemId } from "@calternal/plugin-sdk";

/** The open chat supplied by the harness place and the starter's chosen scope. */
export interface StartInput {
  chat: ItemId;
  scope?: "session" | "item" | "items";
}

/** Starts the adapter with the open chat as its sole target. */
export const start = on.call("runs.start", async ({ chat, scope = "session" }: StartInput, s) => {
  const run = await s.run("calternal-provider-claude-code", { scope, targets: [chat] });
  return { id: run.id };
});
