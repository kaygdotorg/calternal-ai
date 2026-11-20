import type { ItemRef } from "../generated/wire.js";

/** A version 1 harness receives only the checked prompt. */
export type HarnessInput = { text: string };
/** A version 1 harness returns reply text and a checked reference to its own run log. */
export type HarnessOutput = { reply: string; log: ItemRef };

export const catalogueRef = (type: string) => ({ $ref: `host-catalogue-v2.json#/$defs/${type}` });
export const object = <const P extends Record<string, object>, const K extends (keyof P)[]>(properties: P, ...required: K) =>
  ({ type: "object" as const, additionalProperties: false, properties, required });
export const textSchema = { type: "string", minLength: 1, maxLength: 16384 };
export const harnessInput = object({ text: textSchema }, "text");
export const harnessOutput = object({ reply: textSchema, log: catalogueRef("ItemRef") }, "reply", "log");

const MAX_TEXT_BYTES = 16 * 1024;

/** Enforce the prompt and reply byte bound before saving user content. */
export function checkedText(text: string): string {
  if (!text.trim() || /[\u0000-\u0008\u000b\u000c\u000e-\u001f]/u.test(text) || Buffer.byteLength(text) > MAX_TEXT_BYTES) {
    throw new Error("Enter nonblank text of up to 16 KiB without control characters.");
  }
  return text;
}
