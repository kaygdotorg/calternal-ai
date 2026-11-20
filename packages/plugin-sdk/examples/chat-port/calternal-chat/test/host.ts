import { FakeHost } from "@calternal/plugin-sdk/testing";
import harness from "../../calternal-chat-claude-code/src/plugin.ts";
import plugin from "../src/plugin.ts";
import { decode, decodeIndexed } from "../src/format.ts";

/** The SDK fake host with Chat's two packages, a scripted Claude provider and Chat-format readers. */
export class ChatHost extends FakeHost {
  /** Scripted provider reply; reassign per test. */
  claude = async (text: string) => `Reply to ${text}`;

  constructor() {
    super([plugin, harness]);
    this.provide("calternal-chat-claude-code", { claude: (text: string) => this.claude(text) });
    this.execution.readWaitMs = 5;
  }

  /** Decode the head with its committed span index, as Chat's reader does. */
  thread(item: string) {
    const stored = this.item(item);
    const index = stored.index.get(stored.head)!;
    const bytes = stored.revisions.get(stored.head)!;
    return index.spans.length ? decodeIndexed(bytes, index.spans, index.body_provenance).thread : decode(bytes.toString());
  }
}

/** A host with one new chat and a checked agent choice. */
export async function setup() {
  const host = new ChatHost();
  const { created: item } = await host.invoke("chat.create", { title: "Review the lease" }, { captured: true, request_id: "create-1" }) as { created: string };
  const selection = host.choose();
  const send = (request_id: string, text: string) =>
    host.invoke("chat.send", { item, text, ...selection }, { captured: true, request_id }) as Promise<{ job_id: string }>;
  return { host, item, send, selection };
}
