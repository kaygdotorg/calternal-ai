// Test-only protocol worker exercising the ordinary write and prepare broker calls.
'use strict';

const readline = require('node:readline').createInterface({ input: process.stdin });
const pending = new Map();
function send(id, message) {
  process.stdout.write(JSON.stringify({ protocol_version: 2, invocation_id: id, message }) + '\n');
}

readline.on('line', line => {
  const envelope = JSON.parse(line);
  if (envelope.type === 'ready') {
    process.stdout.write(line + '\n');
    return;
  }
  const { invocation_id: id, message } = envelope;
  if (message.type === 'invoke') {
    pending.set(id, message.authority);
    send(id, { type: 'host_call', call_id: 'write', method: 'write_item',
      input: { kind: 'fixture-html', text: message.input.text }, authority: message.authority });
  } else if (message.type === 'host_reply' && pending.has(id)) {
    if (!message.ok) {
      pending.delete(id);
      send(id, { type: 'failed' });
    } else if (message.call_id === 'write') {
      send(id, { type: 'host_call', call_id: 'prepare', method: 'content_prepare_v1',
        input: { kind: 'fixture-html', field: 'text', item_id: message.data.id, revision: message.data.revision },
        authority: pending.get(id) });
    } else {
      pending.delete(id);
      send(id, { type: 'complete', data: message.data });
    }
  } else if (message.type === 'cancel') {
    pending.delete(id);
  }
});
