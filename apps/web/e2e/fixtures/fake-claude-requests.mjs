#!/usr/bin/env node
// A stand-in `claude` for the requests end-to-end run (e2e/requests-server.mjs): selected by words in the prompt, it raises a question, an approval or a secret
// prompt with the turn's own agent credential, or runs a long tool step. After a request is resolved the machine runs it again; a marker file in the workspace
// then lets it finish. Sign-in state is the file named by FAKE_SIGNED_IN (so the test can sign the provider out and in). Prints nothing private.
import { existsSync, readdirSync, writeFileSync } from "node:fs";
import { appendFileSync } from "node:fs";
const args = process.argv.slice(2);
if (process.env.FAKE_LOG) appendFileSync(process.env.FAKE_LOG, `${new Date().toISOString()} ${args.join(" ").slice(0, 120)}\n`);
if (args.join(" ") === "auth status") { console.log(JSON.stringify({ authMethod: "claude.ai" })); process.exit(existsSync(process.env.FAKE_SIGNED_IN ?? "/nonexistent") ? 0 : 1); }
if (process.env.FAKE_LOG) appendFileSync(process.env.FAKE_LOG, `cwd ${process.cwd()} env ${Object.keys(process.env).filter((k) => /CALTERNAL|THREAD|JOB/.test(k)).join(",")}\n`);
let prompt = ""; for await (const d of process.stdin) prompt += d;
const emit = (m) => process.stdout.write(JSON.stringify(m) + "\n");
const text = (t) => emit({ type: "stream_event", event: { type: "content_block_delta", delta: { type: "text_delta", text: t } } });
emit({ type: "system", subtype: "init", session_id: "22222222-2222-4222-8222-222222222222" });
const call = async (operation, input) => {
  const j = await send(operation, input); if (!j.ok) { appendFileSync(process.env.FAKE_LOG, `${operation} -> ${JSON.stringify(j).slice(0, 300)}\n`); process.exit(72); }
  return j;
};
const send = async (operation, input) => {
  const r = await fetch(new URL("/api/v1/operations", process.env.CALTERNAL_URL), { method: "POST", headers: { Authorization: "Bearer " + process.env.CALTERNAL_AGENT_TOKEN, "Content-Type": "application/json" }, body: JSON.stringify({ operation, request_id: crypto.randomUUID(), input }) });
  return r.json();
};
// The turn's own job id: the workspace path ends in .../threads/<thread id>/work, and the thread names its running job.
const jobId = async () => { const id = process.cwd().split("/").slice(-2)[0]; const g = await call("threads.get", { id, space_id: process.env.FAKE_SPACE }); return g.data.job?.id ?? g.data.thread?.job?.id; };
const kinds = { ask: () => call("requests.ask", { form: { genui_version: 2, source: '$release=""\nForm([Input($release,"Which release?")], "Release selection")' } }),
  approve: async () => call("access.request", { space_id: process.env.FAKE_SPACE, job_id: await jobId(), app: "notes", write: true }),
  secret: () => call("secrets.request", { space_id: process.env.FAKE_SPACE, name: "forgejo.token", label: "Forgejo access token", reason: "To publish pages." }) };
const kind = Object.keys(kinds).find((k) => prompt.includes(`[${k}]`));
const marker = `.raised-${kind}`;
if (kind && !existsSync(marker)) {
  writeFileSync(marker, "x");
  emit({ type: "assistant", message: { content: [{ type: "tool_use", id: "t1", name: "Bash", input: { command: "true" } }] } });
  await kinds[kind]();
  setInterval(() => {}, 1000); await new Promise(() => {});
}
if (prompt.includes("[hang]")) { text("Working on it…\n"); emit({ type: "assistant", message: { content: [{ type: "tool_use", id: "t2", name: "Bash", input: { command: "sleep 600" } }] } }); setInterval(() => {}, 1000); await new Promise(() => {}); }
// A resumed run carries the person's answer, not the original words: the marker says a request was raised here.
text(readdirSync(".").some((f) => f.startsWith(".raised-")) ? "Thanks, continuing.\n" : "ready\n");
emit({ type: "result", subtype: "success", is_error: false, usage: { input_tokens: 10, output_tokens: 3 } });
