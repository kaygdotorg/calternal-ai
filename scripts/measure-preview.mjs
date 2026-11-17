// Local smoke measurements, never a production performance gate.
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, writeFile, mkdir, rm } from 'node:fs/promises';
import { tmpdir, cpus, totalmem, platform, release } from 'node:os';
import { join, resolve } from 'node:path';
import { randomUUID, createHash } from 'node:crypto';
import { performance } from 'node:perf_hooks';

const root = resolve(import.meta.dirname, '..');
const data = await mkdtemp(join(tmpdir(), 'calternal-smoke-'));
const started = performance.now();
const child = spawn(join(root, 'target/debug/calternal'), ['serve', '--data-dir', data, '--port', '0', '--plugin-root', join(root, 'plugins')], { stdio: 'ignore' });
let exited = false;
child.on('exit', () => { exited = true; });
let spawnError;
child.on('error', e => { spawnError = e; exited = true; });
const pause = ms => new Promise(r => setTimeout(r, ms));
try {
  let ready;
  for (let i = 0; i < 200; i++) {
    if (spawnError) throw spawnError;
    if (exited) throw new Error('Smoke server exited before readiness');
    try { ready = JSON.parse(await readFile(join(data, 'startup.json'), 'utf8')); break; } catch {}
    await pause(25);
  }
  if (!ready) throw new Error('Smoke server readiness timeout');
  const startupMs = performance.now() - started;
  const token = await readFile(ready.token_files.owner, 'utf8');
  const described = await fetch(`${ready.base_url}/api/v1/describe`, { headers: { Authorization: `Bearer ${token}` } }).then(r => r.json());
  // Any space the owner holds; describe is the only source of space IDs.
  const space = described.principal.spaces[0].id;
  const op = async (operation, input) => {
    const response = await fetch(`${ready.base_url}/api/v1/operations`, { method: 'POST', headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' }, body: JSON.stringify({ request_id: randomUUID(), operation, input }) });
    const result = await response.json();
    if (!response.ok || !result.ok) throw new Error(`${operation}: ${result.error?.code ?? response.status}`);
    return result.data;
  };
  const samples = [];
  const measure = async (name, n, fn) => {
    const values = [];
    for (let i = 0; i < n; i++) { const t = performance.now(); await fn(); values.push(performance.now() - t); }
    values.sort((a, b) => a - b);
    const percentile = p => values[Math.min(values.length - 1, Math.ceil(values.length * p) - 1)];
    samples.push({ name, status: 'measured', metrics: { samples: n, p50_ms: percentile(.5), p95_ms: percentile(.95), p99_ms: percentile(.99), max_ms: values.at(-1) } });
  };
  const markdown = '# Disposable measurement fixture\n\n' + 'Calternal writes Markdown and validates operations.\n'.repeat(200);
  let { note } = await op('notes.create', { space_id: space, title: 'Smoke fixture', markdown });
  for (let i = 0; i < 5; i++) await op('notes.get', { space_id: space, id: note.id });
  await measure('Notes read (warm HTTP, one ~10 KiB note)', 40, () => op('notes.get', { space_id: space, id: note.id }));
  let revision = 0;
  await measure('Notes update (durable acknowledgement)', 20, async () => { ({ note } = await op('notes.update', { space_id: space, id: note.id, base_revision: note.revision, markdown: markdown + `\nRevision ${++revision}\n` })); });
  await op('plugins.enable', { space_id: space, plugin_id: 'notes-insights' });
  await measure('Extension (fresh Node worker per call)', 15, () => op('notes-insights.summarize', { space_id: space, id: note.id }));
  const binarySha256 = createHash('sha256').update(await readFile(join(root, 'target/debug/calternal'))).digest('hex');
  const report = { binary_sha256: binarySha256, measured_at: new Date().toISOString(), scope: 'Development-host smoke only; debug binary, small disposable fixture, no UI or 120 Hz claim; concurrent builds may affect results.', conditions: { platform: platform(), release: release(), cpu: cpus()[0]?.model, logical_cpus: cpus().length, memory_bytes: totalmem(), node: process.version, startup_ms: startupMs, document_utf8_bytes: Buffer.byteLength(markdown) }, results: samples };
  await mkdir(join(root, 'artifacts'), { recursive: true });
  await writeFile(join(root, 'artifacts/preview-smoke.json'), JSON.stringify(report, null, 2) + '\n');
  console.log('Smoke measurements saved to artifacts/preview-smoke.json (not a release gate).');
} finally {
  if (!exited) child.kill('SIGTERM');
  for (let i = 0; i < 100 && !exited; i++) await pause(25);
  if (!exited) { child.kill('SIGKILL'); await new Promise(r => child.once('exit', r)); }
  await rm(data, { recursive: true, force: true });
}
