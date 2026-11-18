// A stand-in for the server's wire protocol, for what the real server cannot hold yet (a 2 MiB thread file ceiling, no way to place a file by hand): a thread of 10,000 messages, a thread
// with one 5 MB agent message, and a turn that streams into the bottom of either. It speaks the shapes the adapter reads (docs/threads.md, docs/dev-e2e.md): operations, the space journal
// (`/api/v1/live`) and the job stream (`/api/v1/jobs/<id>/events`). Usage: node e2e/server-double.mjs <port>   (CAL_SERVER=http://127.0.0.1:<port> for the web preview's proxy)
import http from "node:http";
const PORT = +(process.argv[2] ?? 34200);
const SPACE = "0197f3a2-5c1e-7b4d-9a60-3d2e8f1c4b11";
const uuid = (n) => `019a8c00-0000-7000-8000-${String(n).padStart(12, "0")}`;
const T10K = uuid(1), T5MB = uuid(2);
const t0 = Date.parse("2026-11-17T10:00:00.000Z");
const stamp = (i) => new Date(t0 + i * 60_000).toISOString();
const frontmatter = (title) => `---\ntitle: "${title}"\nprovider: "calternal-claude-code"\nmode: "approve_for_me"\ncreated-at: "${stamp(0)}"\nupdated-at: "${stamp(1)}"\n---\n\n`;
function msg(i, role, text) { return `<!-- message:${uuid(100000 + i)} role:${role} time:${stamp(i)} -->\n${text}\n<!-- /message -->\n`; }
const words = "the journal replays every entry after the restart and the index rebuilds from it without touching the receipts store so a retry finds its answer".split(" ");
const para = (i) => `Message ${i}: ` + Array.from({ length: 20 + (i % 30) }, (_, k) => words[(i + k) % words.length]).join(" ") + ".";
const long = (n) => { let s = frontmatter("Journal replay is slow after restart (10,000 messages)"); for (let i = 0; i < n; i++) s += msg(i, i % 2 ? "agent" : "person", para(i)); return s; };
const bigTool = () => { let s = frontmatter("A 5 MB tool output"); s += msg(0, "person", "Run the whole build and show me everything."); const line = "   Compiling calternal-content v0.1.0 (/home/agent/work/crates/content) warning: unused variable `x` in src/journal.rs\n"; let out = "```\n"; while (out.length < 5 * 1024 * 1024) out += line; out += "```"; s += msg(1, "agent", out); return s; };
const files = { [T10K]: { title: "Journal replay is slow after restart (10,000 messages)", md: long(10_000) }, [T5MB]: { title: "A 5 MB tool output", md: bigTool() } };
const meta = (id) => ({ title: files[id].title, provider: "calternal-claude-code", model: null, mode: "approve_for_me", mode_for: "acc", message_count: (files[id].md.match(/<!-- message:/g) || []).length, last_activity_at: stamp(10_000), created_at: stamp(0), archived: false, deleted: false });
const state = { turn: { thread: null, job: null, timer: null } }; // one streaming turn at a time
const feeds = new Set();
const sendFeed = (kind, item, data = {}) => { for (const r of feeds) r.write(`data: ${JSON.stringify({ type: "entry", space_id: SPACE, entry: { kind, item, data } })}\n\n`); };
const view = (id) => ({ thread: { id, space_id: SPACE, address: `/threads/${id}`, revision: revOf(id), turn_state: state.turn.thread === id ? "running" : "idle", metadata: meta(id), markdown: files[id].md }, ...(state.turn.thread === id ? { job: { id: state.turn.job, state: "running" } } : {}) });
// Revisions and checkpoint additions as the real server keeps them (docs/threads.md, "Read one checkpoint addition"): every save has a revision; `deltas` answers `since` for the save that reached it.
let rev = 0; const revs = {}; const deltas = new Map();
const revOf = (id) => (revs[id] ??= `r${++rev}`);
const bodyBytes = (id) => Buffer.byteLength(files[id].md) - Buffer.byteLength(files[id].md.slice(0, files[id].md.indexOf("\n---\n") + 5));
/** One checkpoint: `text` is appended at the end of the body; the journal entry carries the range, the revision it reached and the revision it started from. */
function save(id, text) {
  const f = files[id]; const base = revOf(id); const offset = bodyBytes(id); const start = Buffer.byteLength(f.md) ; f.md += text; revs[id] = `r${++rev}`;
  deltas.set(revs[id], { id, base_revision: base, revision: revs[id], offset, markdown: text });
  sendFeed("threads.checkpoint", id, { revision: revs[id], append: { base_revision: base, offset, start, length: Buffer.byteLength(text) } });
}
function startTurn(id, seconds) {
  const job = `job-${Date.now()}`; state.turn = { thread: id, job, timer: null };
  const mid = uuid(900000 + Math.floor(Math.random() * 1e5));
  sendFeed("threads.turn_state_changed", id, { job_id: job, state: "running", job_state: "running" });
  save(id, `<!-- message:${mid} role:agent time:${new Date().toISOString()} turn:${job} -->\n`);
  let n = 0, pend = ""; const begin = Date.now();
  state.turn.timer = setInterval(() => {
    n++; pend += (n === 1 ? "" : " ") + `streamed words number ${n} for the person to read as it arrives`;
    if (n % 5 === 0) { save(id, pend); pend = ""; }
    if (Date.now() - begin > seconds * 1000) { clearInterval(state.turn.timer); save(id, pend + "\n<!-- /message -->\n"); sendFeed("threads.turn_state_changed", id, { job_id: job, state: "idle", job_state: "done" }); state.turn = { thread: null, job: null, timer: null }; }
  }, 100);
  return job;
}
const json = (res, v, status = 200) => { res.writeHead(status, { "content-type": "application/json" }); res.end(JSON.stringify(v)); };
http.createServer((req, res) => {
  const url = new URL(req.url, "http://x");
  if (url.pathname === "/api/v1/describe") return json(res, { protocol_version: 1, genui_version: 2, principal: { id: "p", spaces: [] }, operations: [] });
  if (url.pathname === "/api/v1/auth/me") return json(res, { user: { id: "p", name: "Kay", username: "kay", role: "owner", capabilities: [] }, installation: "i" });
  if (url.pathname === "/api/v1/live") { res.writeHead(200, { "content-type": "text/event-stream" }); res.write(`data: ${JSON.stringify({ type: "ready", space_id: SPACE, bookmark: "b0" })}\n\n`); feeds.add(res); req.on("close", () => feeds.delete(res)); return; }
  const jm = /^\/api\/v1\/jobs\/([^/]+)\/events$/.exec(url.pathname);
  if (jm) { res.writeHead(200, { "content-type": "text/event-stream" }); res.write(`event: snapshot\ndata: ${JSON.stringify({ job: { id: jm[1], state: "running" }, epoch: "e" })}\n\n`); let s = 0; const t = setInterval(() => { if (state.turn.job !== jm[1]) { res.write(`event: end\ndata: ${JSON.stringify({ job: { id: jm[1], state: "done" } })}\n\n`); clearInterval(t); res.end(); return; } s++; res.write(`id: e:${s}\nevent: chunk\ndata: ${JSON.stringify({ seq: s, data: { version: 1, seq: s, event: s % 20 === 0 ? { type: "tool_started", summary: `Read file ${s}` } : s % 20 === 1 ? { type: "tool_finished", failed: false } : { type: "text_delta", text: "word " } } })}\n\n`); }, 100); req.on("close", () => clearInterval(t)); return; }
  if (url.pathname === "/api/v1/operations" && req.method === "POST") {
    let body = ""; req.on("data", (c) => (body += c)); req.on("end", () => {
      const { operation, input } = JSON.parse(body); const ok = (data) => json(res, { request_id: "r", ok: true, data });
      if (operation === "spaces.list") return ok({ spaces: [{ id: SPACE, name: "Personal" }] });
      if (operation === "threads.list") return ok({ threads: (process.env.EMPTY ? [] : [T10K, T5MB]).map((id) => ({ id, address: `/threads/${id}`, revision: "r", turn_state: state.turn.thread === id ? "running" : "idle", metadata: meta(id) })) });
      if (operation === "threads.get") { if (input.since) { const d = deltas.get(input.since.revision); return d && d.base_revision === input.since.append.base_revision ? ok({ delta: d }) : json(res, { request_id: "r", ok: false, error: { code: "conflict", message: "checkpoint delta unavailable", details: { reason: "checkpoint_unavailable" } } }, 409); } return ok(view(input.id)); }
      if (operation === "threads.send") { files[input.id].md += msg(20000, "person", input.message); revs[input.id] = `r${++rev}`; sendFeed("threads.send", input.id, {}); const job = startTurn(input.id, +(process.env.TURN_SECONDS ?? 20)); return ok({ ...view(input.id), job: { id: job, state: "queued" } }); }
      return json(res, { request_id: "r", ok: false, error: { code: "not_found", message: "unknown operation" } }, 404);
    }); return;
  }
  res.writeHead(404); res.end();
}).listen(PORT, "127.0.0.1", () => console.log(`double on ${PORT}: 10k thread ${T10K}, 5 MB thread ${T5MB}; file sizes ${Object.values(files).map((f) => (f.md.length / 1048576).toFixed(2) + " MiB").join(", ")}`));
