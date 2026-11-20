import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test, { after } from "node:test";
import { decodeLegacy, legacyMessage, legacyPage } from "../src/legacy.ts";
import { FakeHost, handlers, checkTrace } from "./host.ts";

after(checkTrace);

const sample = (name: string) => readFileSync(new URL(`./fixtures/v1/${name}.md`, import.meta.url));
const disk = sample("tool-output");
const personId = "01a1f88f-9dcd-7747-87a0-45dc2959a445";
const agentId = "01a1f88f-9dd8-7134-b082-60d04dba4a6b";

test("the current server's v1 records retain their headings, IDs, UTF-8 bodies and saved tool notices", () => {
  assert.deepEqual(decodeLegacy(sample("empty")).messages, []);
  const parsed = decodeLegacy(disk);
  assert.deepEqual(parsed, decodeLegacy(sample("tool-output.public")));
  const [prompt, reply] = parsed.messages;
  assert.equal(parsed.messages.length, 2);
  assert.deepEqual(prompt, {
    id: personId,
    role: "person",
    heading: "owner · 2026-11-20 16:15:24 +05:30",
    posted_at: "2026-11-20T10:45:24Z",
    text: "scripted-tool-steps long-output 🌏\n\n## A literal heading\n<!-- id:123 role:person bytes:0 -->\n\n[Agenda](Agenda.md)",
  });
  assert.equal(reply.id, agentId);
  assert.equal(reply.turn, agentId);
  assert.equal(reply.role, "agent");
  assert.equal(reply.posted_at, prompt.posted_at);
  assert.match(reply.heading, /Claude Code via Unknown provider · Sonnet 5.5 · Provider default/);
  assert.match(reply.text, /Hello 🌏/);
  assert.match(reply.text, /Run ls \(98326 bytes of tool output\)/);
  assert.match(reply.text, /Read .+ \(failed\)/);
  assert.doesNotMatch(JSON.stringify(parsed), /provider-session|machine_id|provenance/);
});

test("installed Threads opens a v1 message by its original UUID and refuses v1 mutations", async () => {
  const host = new FakeHost();
  const { created: item } = await handlers["chat.create"]({ title: "Legacy fixture" }, host.createContext("create-legacy", "Legacy fixture"));
  const { publish } = await host.stageUnproven(item, disk);
  await publish();
  const file = host.files.get(item)!;
  const revision = file.revision;
  const open = (anchor?: string, older?: string) =>
    handlers["chat.refresh"]({ item, source_revision: revision, anchor, older }, host.context("v1-read", "chat.refresh"));
  const latest = await open();
  assert.match(JSON.stringify(latest), new RegExp(agentId));
  assert.match(JSON.stringify(latest), /bytes of tool output/);
  assert.ok(latest.older);
  const older = await open(undefined, latest.older);
  assert.match(JSON.stringify(older), new RegExp(personId));
  assert.match(JSON.stringify(older), /Agenda\.md/);
  assert.doesNotMatch(JSON.stringify(latest), /provider-session|machine_id/);
  assert.equal((await open(personId)).anchor, personId);
  assert.deepEqual(await open("unknown"), { anchor_unavailable: true });
  assert.deepEqual(await handlers["chat.message"]({ item, message: personId }, host.context("v1-message", "chat.message")),
    decodeLegacy(disk).messages[0]);
  assert.equal(file.discovery, "consent_only");

  const selection = host.choose();
  await assert.rejects(handlers["chat.send"]({ item, text: "No write", ...selection },
    host.composerContext("v1-no-send", item, "No write")), /read-only/);
  await assert.rejects(handlers["chat.arm"]({ item, text: "No alarm", harness: selection.harness,
    at: "2026-11-20T12:01:00Z", if_missed: "skip" }, host.context("v1-no-alarm", "chat.arm")), /read-only/);
  assert.equal(host.jobs.size, 0);
  assert.equal(file.revision, revision);
  assert.deepEqual(Buffer.from(host.markdown(item)), disk);
});

test("legacy framing rejects duplicates and lengths inside a multibyte character", () => {
  const file = disk.toString();
  const duplicate = file.replace(agentId, personId);
  const byteCount = Buffer.byteLength("scripted-tool-steps long-output ") + 2;
  const partial = file.replace("bytes:115", `bytes:${byteCount}`);
  assert.throws(() => decodeLegacy(Buffer.from(duplicate)), /version 1 thread/);
  assert.throws(() => decodeLegacy(Buffer.from(partial)), /utf-8/i);
});

test("large legacy records page to completion without splitting an emoji", () => {
  const thread = decodeLegacy(disk);
  const reply = thread.messages[1];
  const text = "x" + "🌏".repeat(9000);
  // Keep the real envelope and recorded heading. Resize only the last record for this boundary test.
  const original = disk.toString();
  const start = original.length - reply.text.length - 1;
  const expanded = decodeLegacy(Buffer.from(original.slice(0, start).replace("bytes:319", `bytes:${Buffer.byteLength(text)}`) + text + "\n"));
  const pages: string[] = [];
  for (let offset = 0; ; ) {
    const chunk = legacyMessage(expanded, agentId, offset);
    pages.push(chunk.text);
    if (chunk.next_offset === undefined) break;
    offset = chunk.next_offset;
  }
  assert.equal(pages.join(""), text);
  const end = legacyPage(expanded, undefined, agentId);
  assert.equal(end.anchor, agentId);
  assert.ok(end.older);
  const middle = legacyPage(expanded, end.older);
  assert.ok(middle.older);
  const beginning = legacyPage(expanded, middle.older);
  assert.match(JSON.stringify(beginning), /x🌏/);
  assert.match(JSON.stringify(legacyPage(expanded, beginning.older)), new RegExp(personId));
});
