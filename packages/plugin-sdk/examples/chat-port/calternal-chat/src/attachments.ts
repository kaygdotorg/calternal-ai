import type {
  ExecutionJobReceipt, ExecutionJobStart, ItemAppend, ItemAppendReceipt, ItemRef, ItemViewAssetRef, RevisionRef,
} from "@calternal/plugin-sdk/types";

/** Limit the number of generated file links per message. The 512-byte framing limit also applies. */
export const maxMessageFiles = 4;

/** Portable file identity and label written with a normal conversation-relative Markdown link. */
export type MessageFile = { id: string; name: string };

/** Proposed checked host exchanges for the attachment spike. The 14.43 catalogue pins renderer upload. The worker claim and projection use the fixture. */
export type AttachmentPort = {
  prepare(item: ItemRef, staged: readonly string[]): Promise<readonly MessageFile[]>;
  append(input: ItemAppend, staged: readonly string[]): Promise<ItemAppendReceipt>;
  project(item: ItemRef, revision: RevisionRef, files: readonly MessageFile[]): Promise<readonly { ref: ItemViewAssetRef; label: string }[]>;
  startJob(input: ExecutionJobStart, staged: readonly string[]): Promise<ExecutionJobReceipt>;
  jobFiles?: readonly string[];
};

/** Validate the number and uniqueness of selected upload refs before asking the host to claim them. */
export function checkSelection(staged: readonly string[]) {
  if (staged.length > maxMessageFiles || new Set(staged).size !== staged.length) throw new Error("Choose up to four distinct files.");
}

/** Render a safe link under the conversation folder using the host file identity. */
export function fileLink({ id, name }: MessageFile): string {
  if (!/^[a-f0-9]{16}$/u.test(id) || !name || /[\/\\\p{Cc}]/u.test(name) || name === "." || name === "..") {
    throw new Error("The attachment link is invalid.");
  }
  return `[${name.replace(/[\\[\]*_`<>&]/gu, "\\$&")}](<Attachments/${id}/${encodeURIComponent(name)}>)`;
}

/** Write the consecutive links in the message's generated footer. */
export function fileLinks(files: readonly MessageFile[]) {
  return files.map(fileLink).join("\n");
}

/** Decode a generated link without treating it as a grant. */
export function readFileLink(line: string): MessageFile {
  const parts = /\]\(<Attachments\/([a-f0-9]{16})\/([^/<>]+)>\)$/u.exec(line);
  if (!parts) throw new Error("The attachment link is invalid.");
  const file = { id: parts[1], name: decodeURIComponent(parts[2]) };
  if (fileLink(file) !== line) throw new Error("The attachment link is invalid.");
  return file;
}
