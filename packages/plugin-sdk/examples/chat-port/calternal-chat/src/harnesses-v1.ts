import { object, ref, string } from "@calternal/plugin-sdk";
import type { ItemLocator } from "@calternal/plugin-sdk/types";

/** A version 1 harness receives only the checked prompt. */
export type HarnessInput = { text: string };
/** A version 1 harness returns reply text and a checked reference to its own run log. */
export type HarnessOutput = { text: string; log: ItemLocator };

export const textSchema = string({ minLength: 1, maxLength: 16384 });
export const harnessInput = object({ text: textSchema }, "text");
export const harnessOutput = object({ text: textSchema, log: ref("ItemLocator") }, "text", "log");

const MAX_TEXT_BYTES = 16 * 1024;

/** Enforce the prompt and reply byte bound before saving user content. */
export function checkedText(text: string): string {
  if (!text.trim() || /[\u0000-\u0008\u000b\u000c\u000e-\u001f]/u.test(text) || Buffer.byteLength(text) > MAX_TEXT_BYTES) {
    throw new Error("Enter nonblank text of up to 16 KiB without control characters.");
  }
  return text;
}
