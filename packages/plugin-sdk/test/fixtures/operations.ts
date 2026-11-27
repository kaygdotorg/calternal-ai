import { on, type ItemId } from "@calternal/plugin-sdk";

/** Exercise host calls in one worker event, including a queued pair. */
export const probe = on.call("probe", async ({ item }: { item: ItemId }, s) => {
  const [page, stored] = await Promise.all([s.query({ from: "test.items" }), s.read(item)]);
  const state = await s.state.get(item, "cursor");
  await s.state.set(item, "cursor", 7);
  const value = await s.call("test-other.inspect", { item });
  const run = await s.run("test-harness", { scope: "session", targets: [item] });
  await run.send("hello");
  await run.stop();
  return { page, stored, state, value, me: s.me, space: s.space, event: s.event, status: run.status, pid: process.pid };
});

/** React to a change with two writes separated by a read. */
export const changed = on.change("test.items", async (item: ItemId, s) => {
  await s.write([{ change: { item, fields: { title: "One" } } }]);
  await s.read(item);
  await s.write([{ change: { item, fields: { title: "Two" } } }]);
});

/** Simulate a plugin dying while the route still owns its atomic change list. */
export const crash = on.call("crash", async ({ item }: { item: ItemId }, s) => {
  setTimeout(() => process.exit(23), 300);
  await s.write([
    { change: { item, fields: { title: "partial" } } },
    { "append record": { item, record: { body: "partial" } } },
  ]);
});

/** Have an exported handler that cannot be reached through the host declaration. */
export const hidden = on.call("hidden", () => "not declared");

/** The send-and-start contract before the Chat fixture is amended. */
export const sendWithHarness = on.call("send-with-harness", async (
  { chat, draft, harness }: { chat: ItemId; draft: string; harness: string },
  s,
) => {
  const written = await s.write([{ "append record": { item: chat, record: { body: draft } } }]);
  return s.call(harness, { chat, prompt: written.changes[0].link });
});

/** Route a missing cross-plugin declaration through the same check. */
export const tryUndeclared = on.call("try-undeclared", async (_input, s) =>
  s.call("test-other.hidden", null),
);

/** A reaction cannot start a run, even when it can receive a change. */
export const tryRunOnChange = on.change("test.restricted", async (item: ItemId, s) =>
  s.run("test-harness", { targets: [item] }),
);
