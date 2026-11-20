// Bench-only shared runtime (approach 3): one Node process, one worker thread
// per plugin. Each worker thread is its own V8 isolate with its own heap and
// heap limit. The process multiplexes plugin frames over its stdin/stdout:
//   in:  {"p":N,"f":frame} | {"ctl":"start"|"terminate"|"heap",...}
//   out: {"p":N,"f":frame} | {"ctl":...}
import { Worker } from 'node:worker_threads';
import { createInterface } from 'node:readline';

const workers = new Map();
const out = value => process.stdout.write(JSON.stringify(value) + '\n');

function start({ p, args, heap_mb }) {
  const worker = new Worker(new URL('./thread-boot.mjs', import.meta.url), {
    argv: args, stdin: true, stdout: true, stderr: false,
    resourceLimits: { maxOldGenerationSizeMb: heap_mb },
  });
  workers.set(p, worker);
  createInterface({ input: worker.stdout }).on('line', line => {
    process.stdout.write('{"p":' + p + ',"f":' + line + '}\n');
  });
  worker.on('error', error => out({ ctl: 'error', p, code: error.code ?? null, message: String(error.message).slice(0, 200) }));
  worker.on('exit', code => { workers.delete(p); out({ ctl: 'exit', p, code }); });
  worker.on('online', () => out({ ctl: 'started', p }));
}

createInterface({ input: process.stdin }).on('line', async line => {
  const msg = JSON.parse(line);
  if (msg.f) { workers.get(msg.p)?.stdin.write(JSON.stringify(msg.f) + '\n'); return; }
  if (msg.ctl === 'start') start(msg);
  else if (msg.ctl === 'terminate') {
    const t = process.hrtime.bigint();
    const worker = workers.get(msg.p);
    if (worker) await worker.terminate();
    out({ ctl: 'terminated', p: msg.p, ms: Number(process.hrtime.bigint() - t) / 1e6 });
  } else if (msg.ctl === 'heap') {
    const heaps = [];
    for (const [p, worker] of workers) {
      const h = await worker.getHeapStatistics();
      heaps.push({ p, used_mib: h.used_heap_size / 2 ** 20, total_mib: h.total_heap_size / 2 ** 20, limit_mib: h.heap_size_limit / 2 ** 20, external_mib: h.external_memory / 2 ** 20 });
    }
    const m = process.memoryUsage();
    out({ ctl: 'heap', heaps, main: { rss_mib: m.rss / 2 ** 20, heap_used_mib: m.heapUsed / 2 ** 20, external_mib: m.external / 2 ** 20, array_buffers_mib: m.arrayBuffers / 2 ** 20 } });
  }
});
