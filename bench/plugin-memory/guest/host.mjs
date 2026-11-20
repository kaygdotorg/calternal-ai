// Bench-only supervisor stand-in that runs inside the space microVM. It
// starts the real plugin workers in one of three layouts and answers
// measurement commands, one JSON line per command, on /run/runtime.sock
// (relayed from vsock port 5000) or on stdin with `node host.mjs stdin`.
//
// Layouts:
//   process  one Node process per plugin, all started eagerly (today's design)
//   shared   one Node process for all plugins, one worker thread (isolate) each
//   lazy     start a plugin on its first call and stop it after `idle_ms`;
//            `runtime` selects a process or a worker thread in a shared runtime
import { spawn } from 'node:child_process';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { createServer } from 'node:net';
import { createInterface } from 'node:readline';

const F = process.env.BENCH_FIXTURE ?? '/fixture';
const NODE = process.execPath;
const PROCESS_FLAGS = ['--permission', `--allow-fs-read=${F}`, '--disallow-code-generation-from-strings', '--max-old-space-size=128'];
const SHARED_FLAGS = ['--permission', `--allow-fs-read=${F}`, '--allow-worker', '--disallow-code-generation-from-strings'];
const HEAP_MB = 128;
const runner = `${F}/runner.mjs`;
const CATALOGUE = {
  threads: { args: [runner, `${F}/threads.mjs`], op: 'threads.refresh', input: { item: 'i1', source_revision: 'r1' } },
  'threads-claude-code': { args: [runner, `${F}/threads-claude-code.mjs`], op: 'threads-claude-code.run', input: { text: 'hello' } },
  notes: { args: [`${F}/notes.mjs`], op: 'notes-insights.summarize', input: { id: 'note' } },
  guide: { args: [`${F}/guide.mjs`], op: 'calternal-plugin-guide.explain', input: { topic: 'network' } },
  submission: { args: [`${F}/submission.mjs`], op: 'calternal-plugin-submission-guide.explain', input: {} },
  settings: { args: [`${F}/settings.mjs`], op: 'composer-settings.label', input: {} },
  trivial: { args: [runner, `${F}/trivial.mjs`], op: 'trivial.echo', input: { n: 1 } },
  faults: { args: [runner, `${F}/faults.mjs`], op: 'faults.run', input: { kind: 'ping' } },
};
const NOTE = 'A short note with some words in it.\n'.repeat(60);
const THREAD = readFileSync(`${F}/thread-v1.md`).toString('base64');
const AUTHORITY = { account_id: '01950000-0000-7000-8000-000000000001', space_id: '01950000-0000-7000-8000-000000000002', grant: 'measurement' };

/** In-guest fake broker: fixed replies, no I/O, so timings show the layout only. */
function broker(method) {
  switch (method) {
    case 'read_item': return { text: NOTE };
    case 'read_setting_v1': return 'warmup';
    case 'item_head': return { item: 'i1', revision: 'r1' };
    case 'item_range': return { bytes: THREAD };
    case 'item_create': return { item: 'i2', revision: 'r2', locator: 'l2' };
    case 'item_append': return { revision: 'r3' };
    default: throw new Error('unsupported ' + method);
  }
}

const now = () => process.hrtime.bigint();
const us = t => Number(now() - t) / 1e3;
const sleep = ms => new Promise(r => setTimeout(r, ms));
const events = [];
const forged = [];
const note = (kind, fields) => events.push({ kind, t_ms: Number(now() / 1000000n), ...fields });
let sequence = 0;

/** One plugin's wire state: pending invocations and frame routing. */
class Plugin {
  constructor(name, transport) {
    this.name = name;
    this.spec = CATALOGUE[name];
    this.transport = transport;
    this.pending = new Map();
    this.alive = false;
    this.lastUse = 0;
  }
  frame(f) {
    const request = this.pending.get(f?.invocation_id);
    if (!request) {
      (f?.invocation_id === 'forged' ? forged : events).push({ kind: 'unmatched', plugin: this.name, id: f?.invocation_id ?? null });
      return;
    }
    const m = f.message;
    if (m.type === 'host_call') {
      let reply;
      try { reply = { type: 'host_reply', call_id: m.call_id, ok: true, data: broker(m.method) }; }
      catch { reply = { type: 'host_reply', call_id: m.call_id, ok: false, data: null }; }
      this.transport.send(this, { protocol_version: 2, invocation_id: f.invocation_id, message: reply });
      return;
    }
    this.pending.delete(f.invocation_id);
    request.resolve(m.type === 'complete' ? { ok: true, data: this.name === 'faults' ? m.data : undefined } : { ok: false });
  }
  died(reason) {
    this.alive = false;
    for (const request of this.pending.values()) request.resolve({ ok: false, died: reason });
    this.pending.clear();
  }
  invoke(op = this.spec.op, input = this.spec.input) {
    const id = 'invoke-' + (++sequence);
    this.lastUse = Date.now();
    return new Promise(resolve => {
      this.pending.set(id, { resolve });
      this.transport.send(this, { protocol_version: 2, invocation_id: id, message: { type: 'invoke', operation: op, input, authority: AUTHORITY } });
    });
  }
}

/** Separate-process transport: one sandboxed Node process per plugin. */
class ProcessTransport {
  constructor(env = {}) { this.env = env; this.procs = new Map(); }
  start(plugin) {
    const child = spawn(NODE, [...PROCESS_FLAGS, ...plugin.spec.args], { stdio: ['pipe', 'pipe', 'ignore'], env: { ...process.env, ...this.env } });
    this.procs.set(plugin, child);
    plugin.alive = true;
    createInterface({ input: child.stdout }).on('line', line => {
      let f;
      try { f = JSON.parse(line); } catch { events.push({ kind: 'malformed', plugin: plugin.name }); return; }
      if (f.protocol_version !== 2) { events.push({ kind: 'malformed', plugin: plugin.name, keys: Object.keys(f) }); return; }
      plugin.frame(f);
    });
    child.stdin.on('error', () => {});
    child.exited = new Promise(resolve => child.on('exit', (code, signal) => {
      note('exit', { plugin: plugin.name, code, signal });
      this.procs.delete(plugin);
      plugin.died(signal ?? code);
      resolve();
    }));
  }
  send(plugin, f) { this.procs.get(plugin)?.stdin.write(JSON.stringify(f) + '\n'); }
  async stop(plugin) {
    const child = this.procs.get(plugin);
    if (!child) return 0;
    const t = now();
    child.kill('SIGKILL');
    await child.exited;
    return us(t) / 1e3;
  }
  pids() { return [...this.procs].map(([p, c]) => ({ name: p.name, pid: c.pid })); }
  async close() { await Promise.all([...this.procs.keys()].map(p => this.stop(p))); }
}

/** Shared-runtime transport: one Node process, one worker thread per plugin. */
class SharedTransport {
  constructor() {
    this.byIndex = new Map();
    this.waiters = new Map();
    this.child = spawn(NODE, [...SHARED_FLAGS, `${F}/shared-runtime.mjs`], { stdio: ['pipe', 'pipe', 'ignore'] });
    this.child.stdin.on('error', () => {});
    createInterface({ input: this.child.stdout }).on('line', line => {
      let msg;
      try { msg = JSON.parse(line); } catch { events.push({ kind: 'malformed', plugin: 'shared-runtime' }); return; }
      if (msg.f) { this.byIndex.get(msg.p)?.frame(msg.f); return; }
      const plugin = this.byIndex.get(msg.p);
      if (msg.ctl === 'exit' || msg.ctl === 'error') { note(msg.ctl, { plugin: plugin?.name, code: msg.code, message: msg.message }); }
      if (msg.ctl === 'exit' && plugin) plugin.died('exit ' + msg.code);
      this.waiters.get(msg.ctl + ':' + (msg.p ?? ''))?.(msg);
    });
    this.exited = new Promise(resolve => this.child.on('exit', (code, signal) => {
      note('runtime-exit', { code, signal });
      this.dead = true;
      for (const p of this.byIndex.values()) p.died('runtime ' + (signal ?? code));
      resolve();
    }));
  }
  wait(key) { return new Promise(resolve => this.waiters.set(key, resolve)); }
  ctl(msg) { this.child.stdin.write(JSON.stringify(msg) + '\n'); }
  start(plugin) {
    plugin.index ??= this.byIndex.size;
    this.byIndex.set(plugin.index, plugin);
    plugin.alive = true;
    this.ctl({ ctl: 'start', p: plugin.index, args: plugin.spec.args, heap_mb: HEAP_MB });
  }
  send(plugin, f) { if (!this.dead) this.ctl({ p: plugin.index, f }); }
  async stop(plugin) {
    if (!plugin.alive || this.dead) return 0;
    const t = now();
    const done = this.wait('terminated:' + plugin.index);
    this.ctl({ ctl: 'terminate', p: plugin.index });
    const reply = await done;
    plugin.died('terminated');
    return { host_ms: us(t) / 1e3, runtime_ms: reply.ms };
  }
  async heap() {
    if (this.dead) return null;
    const done = this.wait('heap:');
    this.ctl({ ctl: 'heap' });
    return done;
  }
  pids() { return this.dead ? [] : [{ name: 'shared-runtime', pid: this.child.pid }]; }
  async close() { if (!this.dead) { this.child.kill('SIGKILL'); await this.exited; } }
}

let layout = null;

function smaps(pid) {
  const out = {};
  try {
    for (const line of readFileSync(`/proc/${pid}/smaps_rollup`, 'utf8').split('\n')) {
      const m = /^(\w+):\s+(\d+) kB/.exec(line);
      if (m) out[m[1]] = Number(m[2]) / 1024;
    }
  } catch { return null; }
  return out;
}
function meminfo() {
  const out = {};
  for (const line of readFileSync('/proc/meminfo', 'utf8').split('\n')) {
    const m = /^(\w+):\s+(\d+) kB/.exec(line);
    if (m) out[m[1]] = Number(m[2]) / 1024;
  }
  return out;
}
async function mem() {
  const procs = layout.transport.pids().map(({ name, pid }) => {
    const s = smaps(pid);
    return s && { name, pid, rss: s.Rss, pss: s.Pss, pss_anon: s.Pss_Anon, pss_file: s.Pss_File };
  }).filter(Boolean);
  const host = smaps(process.pid);
  const info = meminfo();
  const sum = key => procs.reduce((total, p) => total + (p[key] ?? 0), 0);
  return {
    procs, plugin_pss_mib: sum('pss'), plugin_rss_mib: sum('rss'), host_pss_mib: host?.Pss,
    guest_used_mib: info.MemTotal - info.MemAvailable, guest_free_mib: info.MemFree, guest_cached_mib: info.Cached,
    heaps: layout.transport.heap ? await layout.transport.heap() : undefined,
  };
}

async function ensure(plugin) {
  if (plugin.alive) return false;
  layout.transport.start(plugin);
  return true;
}
async function call(plugin) {
  const started = await ensure(plugin);
  const t = now();
  const result = await plugin.invoke();
  const elapsed = us(t);
  if (layout.idle_ms) {
    clearTimeout(plugin.idle);
    const transport = layout.transport;
    plugin.idle = setTimeout(() => { note('idle-stop', { plugin: plugin.name }); transport.stop(plugin); }, layout.idle_ms);
  }
  return { us: elapsed, ok: result.ok, started };
}

async function setLayout({ mode, runtime = 'process', plugins, idle_ms = 0, compile_cache = false }) {
  if (layout) { for (const p of layout.plugins.values()) clearTimeout(p.idle); await layout.transport.close(); }
  events.length = 0; forged.length = 0;
  const env = {};
  if (compile_cache) { mkdirSync('/run/ccache', { recursive: true }); env.NODE_COMPILE_CACHE = '/run/ccache'; }
  const shared = mode === 'shared' || (mode === 'lazy' && runtime === 'shared');
  const transport = shared ? new SharedTransport() : new ProcessTransport(env);
  layout = { mode, runtime, idle_ms: mode === 'lazy' ? idle_ms : 0, transport, plugins: new Map() };
  for (const name of plugins) layout.plugins.set(name, new Plugin(name, transport));
  const t = now();
  if (mode !== 'lazy') {
    for (const p of layout.plugins.values()) transport.start(p);
    const results = await Promise.all([...layout.plugins.values()].map(p => p.invoke()));
    if (!results.every(r => r.ok)) throw new Error('a plugin failed its first call: ' + JSON.stringify(results));
  }
  return { ready_ms: us(t) / 1e3 };
}

async function bench({ plugin, n = 1000, warmup = 50 }) {
  const p = layout.plugins.get(plugin);
  for (let i = 0; i < warmup; i++) await call(p);
  const samples = [];
  for (let i = 0; i < n; i++) { const r = await call(p); if (!r.ok) throw new Error('call failed'); samples.push(r.us); }
  return { samples };
}

/** First call after a stop: start, load and answer; then the next calls show warm-up. */
async function coldCall({ plugin, drop_caches = false, following = 20 }) {
  const p = layout.plugins.get(plugin);
  if (p.alive) { clearTimeout(p.idle); await layout.transport.stop(p); }
  if (drop_caches) writeFileSync('/proc/sys/vm/drop_caches', '3');
  const first = await call(p);
  const next = [];
  for (let i = 0; i < following; i++) next.push((await call(p)).us);
  return { first_ms: first.us / 1e3, ok: first.ok, following_us: next };
}

/** Memory over time with idle stop: all plugins used once, then one hot plugin. */
async function timeline({ hot, period_ms = 2000, duration_ms = 60000, sample_ms = 500, extra_at_ms = 30000, extra = 'notes' }) {
  const series = [];
  const t0 = Date.now();
  const sampler = setInterval(async () => {
    const m = await mem();
    series.push({ t_ms: Date.now() - t0, live: m.procs.length, plugin_pss_mib: m.plugin_pss_mib, guest_used_mib: m.guest_used_mib, live_threads: m.heaps?.heaps?.length });
  }, sample_ms);
  await Promise.all([...layout.plugins.values()].map(call));
  let extraDone = false;
  while (Date.now() - t0 < duration_ms) {
    await sleep(period_ms);
    await call(layout.plugins.get(hot));
    if (!extraDone && Date.now() - t0 >= extra_at_ms) { extraDone = true; await call(layout.plugins.get(extra)); }
  }
  clearInterval(sampler);
  return { series, events: [...events] };
}

/** Run one misbehaving plugin while the others keep answering calls. */
async function fault({ kind, target = 0, watch_ms = 3000, kill_after_ms = 1000 }) {
  const faults = layout.plugins.get('faults');
  const others = [...layout.plugins.values()].filter(p => p !== faults);
  const stats = Object.fromEntries(others.map(p => [p.name, { calls: 0, failed: 0, samples: [] }]));
  let running = true;
  const pinger = (async () => {
    while (running) {
      for (const p of others) {
        if (!p.alive) { stats[p.name].failed++; continue; }
        const r = await call(p);
        stats[p.name].calls++;
        if (r.ok) stats[p.name].samples.push(r.us); else stats[p.name].failed++;
      }
      await sleep(5);
    }
  })();
  await sleep(200);
  const t = now();
  let outcome = null;
  const faultCall = faults.invoke('faults.run', { kind, target }).then(r => { outcome = { ...r, after_ms: us(t) / 1e3 }; });
  let stopped = null;
  if (kind === 'loop') {
    await sleep(kill_after_ms);
    stopped = await layout.transport.stop(faults);
  }
  await Promise.race([faultCall, sleep(watch_ms)]);
  await sleep(500);
  running = false;
  await pinger;
  const summary = Object.fromEntries(Object.entries(stats).map(([name, s]) => {
    const sorted = s.samples.sort((a, b) => a - b);
    const q = f => sorted.length ? sorted[Math.min(sorted.length - 1, Math.ceil(f * sorted.length) - 1)] : null;
    return [name, { calls: s.calls, failed: s.failed, p50_us: q(0.5), p95_us: q(0.95), max_us: sorted.at(-1) ?? null, alive: layout.plugins.get(name).alive }];
  }));
  return { kind, outcome, stopped, faults_alive: faults.alive, others: summary, events: [...events], forged: [...forged] };
}

const commands = {
  layout: setLayout, bench, mem, cold_call: coldCall, timeline, fault,
  call: async ({ plugin }) => call(layout.plugins.get(plugin)),
  events: async () => ({ events: [...events], forged: [...forged] }),
  close: async () => { if (layout) { for (const p of layout.plugins.values()) clearTimeout(p.idle); await layout.transport.close(); } layout = null; return {}; },
};

function serve(input, output) {
  let queue = Promise.resolve();
  createInterface({ input }).on('line', line => {
    queue = queue.then(async () => {
      let reply;
      try {
        const msg = JSON.parse(line);
        reply = { result: await commands[msg.op](msg) };
      } catch (error) {
        reply = { error: String(error?.stack ?? error).slice(0, 2000) };
      }
      output.write(JSON.stringify(reply) + '\n');
    });
  });
}

try { writeFileSync('/proc/self/oom_score_adj', '-1000'); } catch { /* not root outside the guest */ }
if (process.argv[2] === 'stdin') serve(process.stdin, process.stdout);
else createServer(s => { s.on('error', () => {}); serve(s, s); }).listen('/run/runtime.sock');
