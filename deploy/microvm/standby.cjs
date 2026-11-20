'use strict';

// Only this trusted module runs before the checkpoint. Drop addon permission
// before loading anything else and keep only this module's private reference.
const addonPath = __dirname + '/reseed.node';
const addon = require(addonPath);
process.permission.drop('addon');
if (process.permission.has('addon') !== false) process.exit(126);
delete require.cache[addonPath];
const addonChild = module.children.findIndex(child => child.filename === addonPath);
if (addonChild === -1) process.exit(126);
module.children.splice(addonChild, 1);
delete process.mainModule;
if (process.mainModule !== undefined) process.exit(126);

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
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
const pluginInput = new PassThrough();
const refreshCommand = Buffer.from('refresh\n');
const loadCommand = Buffer.from('load\n');
let command = refreshCommand;
let matched = 0;

function loadPackage() {
  // The control pipe stays live only while the package reads its stdin.
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
}

function control(chunk) {
  for (let offset = 0; offset < chunk.length; offset++) {
    if (chunk[offset] !== command[matched++]) process.exit(126);
    if (matched !== command.length) continue;
    if (command === refreshCommand) {
      // The agent sends load only after reading the acknowledgement.
      if (offset !== chunk.length - 1) process.exit(126);
      refresh();
      assertDenials();
      command = loadCommand;
      matched = 0;
      process.stdout.write('REFRESHED\n');
    } else {
      controlInput.removeListener('data', control);
      if (offset + 1 < chunk.length) pluginInput.write(chunk.subarray(offset + 1));
      controlInput.pipe(pluginInput);
      loadPackage();
      return;
    }
  }
}
controlInput.on('data', control);
process.stdout.write('STANDBY\n');
