// Test-only protocol-v2 worker (plain JavaScript, no SDK) for the jobs conformance
// cases. Never shipped: it is trusted only by servers built with `test-plugins`.
//
// jobs-demo.write  writes one hidden item.
// jobs-demo.list   returns one broker page.
// jobs-demo.start  admits a job running jobs-demo.run with the same input.
// jobs-demo.run    a leaf streams `steps` chunks `delay_ms` apart, checkpointing every
//                  `every` steps and resuming from the last checkpoint; a parent
//                  (`children` > 0) starts its children, waits, then sums their values.
'use strict';
const rl = require('readline').createInterface({ input: process.stdin });
const send = f => process.stdout.write(JSON.stringify(f) + '\n');
const active = new Map();
let calls = 0;
const frame = (id, message) => ({ protocol_version: 2, invocation_id: id, message });
const sleep = ms => new Promise(r => setTimeout(r, ms));

function call(id, method, input) {
  const s = active.get(id);
  if (!s) return Promise.reject(new Error('cancelled'));
  const call_id = 'c' + calls++;
  return new Promise((resolve, reject) => {
    s.pending.set(call_id, { resolve, reject });
    send(frame(id, { type: 'host_call', call_id, method, input, authority: s.authority }));
  });
}
function chunk(id, job_id, data) {
  const s = active.get(id);
  if (!s) throw new Error('cancelled');
  send(frame(id, { type: 'chunk', job_id, sequence: s.sequence++, data }));
}

const ops = {
  'jobs-demo.list': async (id, input) => call(id, 'list_items_v1', input),
  'jobs-demo.write': async (id, input) => {
    const saved = await call(id, 'write_item', { kind: 'demo-notes', text: input.text });
    if (input.text === 'bad-output-after-write') return { id: 42 };
    if (input.text === 'pause-after-write') await new Promise(() => {});
    return { id: saved.id };
  },
  'jobs-demo.start': async (id, input) => ({ job_id: (await call(id, 'start_job', { operation: 'jobs-demo.run', input })).job_id }),
  'jobs-demo.run': async (id, { job_id, input, checkpoint, children }) => {
    const wanted = input.children || 0;
    if (wanted > 0) {
      if (!checkpoint) {
        for (let k = 1; k <= wanted; k++) {
          await call(id, 'start_job', { operation: 'jobs-demo.run', input: { steps: 2, delay_ms: 10, every: 1, value: k }, parent_id: job_id });
        }
        await call(id, 'checkpoint_job', { job_id, checkpoint: { spawned: wanted } });
      }
      if (children.length < wanted) return { status: 'waiting' };
      const sum = children.reduce((a, c) => a + (c.result && c.result.value || 0), 0);
      return { status: 'done', result: { sum, reports: children.length } };
    }
    const from = checkpoint && checkpoint.step || 0;
    for (let step = from; step < input.steps; ) {
      await sleep(input.delay_ms);
      chunk(id, job_id, { step });
      step++;
      if (step % input.every === 0) await call(id, 'checkpoint_job', { job_id, checkpoint: { step } });
    }
    return { status: 'done', result: { steps: input.steps, resumed_from: from, value: input.value === undefined ? input.steps : input.value } };
  },
};

rl.on('line', line => {
  const f = JSON.parse(line);
  const m = f.message;
  const id = f.invocation_id;
  if (m.type === 'invoke') {
    active.set(id, { authority: m.authority, pending: new Map(), sequence: 0 });
    ops[m.operation](id, m.input).then(
      data => { if (active.delete(id)) send(frame(id, { type: 'complete', data })); },
      () => { if (active.delete(id)) send(frame(id, { type: 'failed' })); },
    );
  } else if (m.type === 'host_reply') {
    const s = active.get(id);
    const p = s && s.pending.get(m.call_id);
    if (p) { s.pending.delete(m.call_id); m.ok ? p.resolve(m.data) : p.reject(new Error('refused')); }
  } else if (m.type === 'cancel') {
    const s = active.get(id);
    active.delete(id);
    if (s) for (const p of s.pending.values()) p.reject(new Error('cancelled'));
  }
});
