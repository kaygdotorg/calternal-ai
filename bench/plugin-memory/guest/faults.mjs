// Bench-only fault plugin. Each kind misbehaves in one way so the bench can
// see what happens to the other plugins in the same layout.
import { fstatSync, readFileSync, writeSync } from 'node:fs';

const keep = [];
export const handlers = {
  'faults.run': async ({ kind, target }) => {
    switch (kind) {
      case 'ping': return { ok: true };
      case 'loop': for (;;) { /* runaway synchronous loop */ }
      case 'heap': {
        // Grow the JS heap until the heap limit stops this plugin.
        for (;;) keep.push(new Array(1e5).fill({ x: keep.length }));
      }
      case 'buffer': {
        // Grow memory outside the JS heap (not covered by heap limits).
        await new Promise(() => setInterval(() => keep.push(Buffer.alloc(16 << 20, 1)), 20));
        break;
      }
      case 'abort': process.abort();
      case 'exit': process.exit(3);
      // A native crash: the signal reaches the whole process, every thread in it.
      case 'kill': process.kill(process.pid, 'SIGSEGV'); return { ok: false };
      case 'throw': setTimeout(() => { throw new Error('uncaught'); }); return { ok: true };
      case 'forge': {
        // Write a frame onto descriptor 1 claiming to come from another plugin.
        const line = JSON.stringify({ p: target, f: { protocol_version: 2, invocation_id: 'forged', message: { type: 'complete', data: 'forged' } } });
        writeSync(1, line + '\n');
        return { wrote: line.length };
      }
      case 'peek': {
        const fds = [];
        for (let fd = 0; fd < 32; fd++) { try { fstatSync(fd); fds.push(fd); } catch { /* closed */ } }
        const tryRead = path => { try { return readFileSync(path).length; } catch (error) { return error.code; } };
        return { fds, maps: tryRead('/proc/self/maps'), environ: tryRead('/proc/self/environ') };
      }
      default: throw new Error('unknown kind');
    }
  },
};
