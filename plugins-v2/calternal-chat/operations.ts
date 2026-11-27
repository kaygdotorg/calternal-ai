import { on, type ItemId, type RecordLink, type ItemLink, type PersonId } from "@calternal/plugin-sdk";

/** The chat to rename and its new title. */
export interface RenameInput {
  chat: ItemId;
  title: string;
}

/** A message and optional references, with its author stamped by the host. */
export interface SendInput {
  chat: ItemId;
  text: string;
  quote?: RecordLink;
  mentions?: PersonId[];
  attachments?: ItemLink[];
  /** Optional requested operation, checked against the host's slot selection. */
  harness?: string;
}

/** Writes only the title and lets the host maintain the item's path and links. */
export const rename = on.call("chats.rename", async ({ chat, title }: RenameInput, s) =>
  s.write([{ change: { item: chat, fields: { title } } }]),
);

/** Appends one complete message. The same operation accepts a fenced harness reply. */
export const send = on.call("chats.send", async ({ chat, text, quote, mentions, attachments, harness }: SendInput, s) => {
  if (!text.trim() && !attachments?.length) {
    throw { code: "invalid", message: "Write a message or attach a file.", next: "Edit the message." };
  }

  const receipt = await s.write([{
    "append record": {
      item: chat,
      record: { fields: { quote, mentions, attachments }, body: text },
    },
  }]);
  const operation = harness ?? s.event.selected_operation;
  if (operation) {
    const prompt = receipt.changes[0].link;
    if (!prompt) {
      throw { code: "unreachable", message: "The record link is unavailable.", next: "Retry the message." };
    }
    await s.call(operation, { chat, prompt });
  }
  return receipt;
});
