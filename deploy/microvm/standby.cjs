'use strict';

// Only this trusted module runs before the checkpoint. Drop addon permission
// before loading anything else and keep only this module's private reference.
const addonPath = __dirname + '/reseed.node';
const addon = require(addonPath);
process.permission.drop('addon');
if (process.permission.has('addon') !== false) process.exit(126);
delete require.cache[addonPath];

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const readline = require('node:readline');
const v8 = require('node:v8');
const { PassThrough } = require('node:stream');
const { spawnSync } = require('node:child_process');
const { Worker } = require('node:worker_threads');

function assertDenials() {
  const copy = __dirname + '/reseed-second.node';
  const denied = { code: 'ERR_ACCESS_DENIED' };
  assert.throws(() => require(copy), denied);
  assert.throws(() => process.dlopen({ exports: {} }, copy), denied);
  assert.throws(() => new Worker('0', { eval: true }), denied);
  assert.throws(() => spawnSync('/bin/true'), denied);
  assert.throws(() => fs.writeFileSync('/tmp/calternal-denied', 'x'), denied);
  assert.throws(() => process.binding('natives'), denied);
}

function refresh() {
  assert.match(addon.reseed(), /\/node$/);
  for (let remaining = 128; remaining > 0; remaining--) crypto.randomUUID();
  const seed = (crypto.randomBytes(4).readUInt32BE() & 0x7fffffff) || 1;
  v8.setFlagsFromString('--random-seed=' + seed);
  const pool = new Uint32Array(128);
  let index = pool.length;
  Math.random = function random() {
    if (index >= pool.length) {
      crypto.getRandomValues(pool);
      index = 0;
    }
    const high = pool[index++] >>> 5;
    const low = pool[index++] >>> 6;
    return (high * 67108864 + low) / 9007199254740992;
  };
}

assertDenials();
// Exercise the filled UUID cache and initialized main-context generator.
crypto.randomUUID();
Math.random();

const controlInput = process.stdin;
const lines = readline.createInterface({ input: controlInput, crlfDelay: Infinity });
const pluginInput = new PassThrough();
let phase = 'parked';
lines.on('line', (line) => {
  if (phase === 'package') {
    pluginInput.write(line + '\n');
  } else if (phase === 'parked' && line === 'refresh') {
    refresh();
    assertDenials();
    phase = 'refreshed';
    process.stdout.write('REFRESHED\n');
  } else if (phase === 'refreshed' && line === 'load') {
    phase = 'package';
    // The control pipe must not keep a one-shot package alive. Reference it
    // when the package attaches a reader, like the original Node stdin.
    controlInput.unref();
    const reads = event => event === 'data' || event === 'readable';
    pluginInput.on('newListener', event => {
      if (reads(event)) controlInput.ref();
    });
    pluginInput.on('removeListener', event => {
      if (reads(event) && !pluginInput.listenerCount('data') && !pluginInput.listenerCount('readable'))
        controlInput.unref();
    });
    Object.defineProperty(process, 'stdin', { value: pluginInput });
    import('/plugin/worker.js').catch((error) => {
      console.error(error);
      process.exit(126);
    });
  } else {
    process.exit(126);
  }
});
lines.on('close', () => pluginInput.end());
process.stdout.write('STANDBY\n');
