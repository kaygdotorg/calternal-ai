// Exercise the run wire protocol on a cookie-authenticated host socket.
import assert from "node:assert/strict";

export const runText = "hold: live log";
export const runKey = "host-run-log";
export const slot = { slot: "harness", filler: "calternal-chat-claude-code" };

export async function verifyRunLog(page, item) {
  const proof = await page.evaluate(async ({ item, runText, runKey, slot }) => {
    const { space } = window.host.session;
    async function call(name, input, extra = {}) {
      const response = await fetch(`/v1/spaces/${space}/call`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ call: name, input, ...extra }),
      });
      const body = await response.json();
      if (!response.ok) throw new Error(`${name}: ${body.error.code}`);
      return body.result;
    }
    async function subscribe(run, after) {
      const socket = new WebSocket(`ws://${location.host}/v1/spaces/${space}/live`);
      const queue = [];
      let notify;
      socket.addEventListener("message", ({ data }) => {
        queue.push(JSON.parse(data));
        notify?.();
      });
      await new Promise((resolve, reject) => {
        socket.onopen = resolve;
        socket.onerror = reject;
      });
      socket.send(JSON.stringify({ type: "subscribe", sub: "run", run_log: { run, after } }));
      return {
        close: () => socket.close(),
        async next() {
          for (;;) {
            while (!queue.length) await new Promise((resolve) => { notify = resolve; });
            const frame = queue.shift();
            if (frame.type === "error") throw new Error(`log: ${frame.code}`);
            if (frame.type === "run_log") return frame;
          }
        },
      };
    }
    async function proof() {
      const { run } = await call("calternal-chat.chats.send", { item, text: runText }, {
        idempotency_key: runKey,
        slot_selection: slot,
      });
      const live = await subscribe(run.id, 0);
      const chunks = [];
      let offset = 0;
      let output = "";
      while (!output.includes(runText)) {
        const frame = await live.next();
        offset = frame.offset;
        for (const chunk of frame.chunks) {
          chunks.push(chunk.offset);
          if (chunk.event === "data" && chunk.stream === "stdout") output += atob(chunk.bytes);
        }
      }
      live.close();
      const start = performance.now();
      await call("s.run.stop", { id: run.id });
      const stop = performance.now() - start;

      const resumed = await subscribe(run.id, offset);
      const suffix = [];
      let stopped = false;
      while (!stopped) {
        const frame = await resumed.next();
        stopped = frame.stopped;
        suffix.push(...frame.chunks);
      }
      resumed.close();
      return { run: run.id, chunks, offset, suffix: suffix.map(({ offset }) => offset), exit: suffix.at(-1)?.event, stop };
    }
    let timer;
    try {
      return await Promise.race([
        proof(),
        new Promise((_, reject) => { timer = setTimeout(() => reject(new Error("run log timed out")), 20_000); }),
      ]);
    } finally {
      clearTimeout(timer);
    }
  }, { item, runText, runKey, slot });
  assert.ok(proof.stop < 2_000, `Stop took ${proof.stop}ms`);
  assert.ok(proof.suffix.length > 0);
  assert.equal(proof.exit, "exit");
  assert.equal(proof.chunks.at(-1), proof.offset);
  const all = [...proof.chunks, ...proof.suffix];
  assert.deepEqual(all, all.map((_, i) => i + 1));
  return proof.run;
}
