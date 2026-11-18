import {spawn} from 'node:child_process';
import {mkdtemp, mkdir, readlink, rm} from 'node:fs/promises';
import {createInterface} from 'node:readline';
import {once} from 'node:events';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import assert from 'node:assert/strict';

const binary = fileURLToPath(new URL('./fake-codex.mjs', import.meta.url));
const adapter = fileURLToPath(new URL('../dist/main.js', import.meta.url));

async function fixture(t, args, extraEnv) {
  const root = await mkdtemp(join(tmpdir(), 'calternal-codex-fixture-'));
  t.after(() => rm(root, {recursive: true, force: true}));
  const home = join(root, 'home');
  const login = join(root, 'login');
  await mkdir(home, {mode: 0o700});
  await mkdir(login, {mode: 0o700});
  const child = spawn(process.execPath, [adapter, ...args], {
    cwd: root,
    env: {
      HOME: home,
      PATH: process.env.PATH,
      CALTERNAL_CODEX_BINARY: binary,
      CALTERNAL_HARNESS_VERSION: '0.170.0',
      CALTERNAL_PROVIDER_LOGIN_DIR: login,
      CALTERNAL_PROVIDER_LOGIN: '1',
      CALTERNAL_AGENT_MODE: 'approve_for_me',
      ...extraEnv,
    },
  });
  t.after(() => child.kill('SIGKILL'));
  return {child, home, login};
}

test('resumes an exact native session then steers and interrupts that active turn', {timeout: 8000}, async t => {
  const {child, home, login} = await fixture(t, []);
  const exited = once(child, 'exit');
  let seq = 0;
  const send = command => child.stdin.write(JSON.stringify({version: 1, seq: ++seq, command}) + '\n');
  send({type: 'start', model: 'gpt-5.5', options: {reasoning: 'high'}, instructions: 'fixture', cwd: '.',
    resume: '01234567-89ab-4cde-8fab-0123456789ab'});
  send({type: 'send', prompt: 'fixture', context: []});
  const events = [];
  for await (const line of createInterface({input: child.stdout})) {
    const {event} = JSON.parse(line);
    events.push(event.type);
    if (event.type === 'session') {
      await new Promise(resolve => setTimeout(resolve, 40));
      send({type: 'steer', prompt: 'steer'});
    }
    if (event.type === 'text_delta') {
      assert.equal(event.text, 'Steered.');
      send({type: 'stop'});
    }
    if (event.type === 'done') assert.equal(event.outcome, 'cancelled');
  }
  assert.deepEqual(await exited, [0, null]);
  assert.deepEqual(events, ['check', 'session', 'text_delta', 'done']);
  assert.equal(await readlink(join(home, '.codex', 'auth.json')), join(login, 'auth.json'));
});

test('a failed account probe reports a check failure without claiming sign-out', {timeout: 8000}, async t => {
  const {child} = await fixture(t, [], {CALTERNAL_URL: 'https://probe-failure.example.invalid'});
  const exited = once(child, 'exit');
  child.stdin.end(JSON.stringify({version: 1, seq: 1, command: {type: 'check'}}) + '\n');
  const events = [];
  for await (const line of createInterface({input: child.stdout})) events.push(JSON.parse(line).event);
  assert.deepEqual(await exited, [0, null]);
  assert.deepEqual(events.map(event => event.type), ['error', 'done']);
  assert.equal(events[0].code, 'provider_check_failed');
  assert.equal(events[1].outcome, 'failed');
  assert.ok(!JSON.stringify(events).includes('PRIVATE_FIXTURE_DETAIL'));
});

test('relays only the private device challenge and confirms official sign-in', {timeout: 8000}, async t => {
  const {child} = await fixture(t, ['--sign-in']);
  const exited = once(child, 'exit');
  const frames = [];
  for await (const line of createInterface({input: child.stdout})) frames.push(JSON.parse(line));
  assert.deepEqual(await exited, [0, null]);
  assert.deepEqual(frames, [{url: 'https://auth.openai.com/codex/device', display_code: 'FIXTURE-0123', needs_code: false}]);
});
