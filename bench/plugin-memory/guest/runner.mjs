// Bench-only adapter: serve a module that exports `handlers` over the
// worker-v2 newline wire on stdin/stdout. Used for the Threads bundles (which
// export handlers and rely on the host's runner) and for the trivial and
// fault fixtures. It runs unchanged as a process or as a worker thread
// (inside a worker thread, process.stdin/stdout are the thread's own streams).
import { createInterface } from 'node:readline';
import { pathToFileURL } from 'node:url';

const modulePath = process.argv[2];
const { handlers } = await import(pathToFileURL(modulePath).href);
const write = frame => process.stdout.write(JSON.stringify(frame) + '\n');
const active = new Map();
let calls = 0;

createInterface({ input: process.stdin }).on('line', line => {
  const { invocation_id: id, message: m } = JSON.parse(line);
  if (m.type === 'host_reply') {
    const pending = active.get(id)?.get(m.call_id);
    if (!pending) throw new Error('unmatched host reply');
    active.get(id).delete(m.call_id);
    if (m.ok) pending.resolve(m.data); else pending.reject(new Error('host call refused'));
    return;
  }
  if (m.type !== 'invoke') return;
  const pending = new Map();
  active.set(id, pending);
  const call = (method, input) => new Promise((resolve, reject) => {
    const callId = 'c' + (++calls);
    pending.set(callId, { resolve, reject });
    write({ protocol_version: 2, invocation_id: id, message: { type: 'host_call', call_id: callId, method, input } });
  });
  const ctx = {
    call,
    author: { request_id: id, actor: 'bench-actor', actor_label: 'Bench', authored_at: '2026-11-20T00:00:00Z' },
    // Fake provider for the Claude Code contribution worker.
    claude: async text => 'reply to ' + text.length + ' characters',
  };
  Promise.resolve()
    .then(() => handlers[m.operation](m.input, ctx))
    .then(
      data => write({ protocol_version: 2, invocation_id: id, message: { type: 'complete', data: data ?? null } }),
      () => write({ protocol_version: 2, invocation_id: id, message: { type: 'failed' } }),
    )
    .finally(() => active.delete(id));
});
