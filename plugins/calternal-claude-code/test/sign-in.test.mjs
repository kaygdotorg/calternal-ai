import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, readFile, rm, stat } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const launcher = fileURLToPath(new URL('../dist/main.js', import.meta.url));
const fake = fileURLToPath(new URL('./fake-claude.mjs', import.meta.url));

test('official sign-in relays only the challenge and keeps credentials on the machine', async () => {
  const root = await mkdtemp(join(tmpdir(), 'calternal-sign-in-'));
  const home = join(root, 'home');
  const shared = join(root, 'login');
  await mkdir(home);
  await mkdir(shared);
  const child = spawn(process.execPath, [launcher, '--sign-in'], {
    env: { HOME: home, PATH: process.env.PATH, CALTERNAL_CLAUDE_LOGIN_DIR: shared,
      CALTERNAL_PROVIDER_LOGIN: '1', CALTERNAL_CLAUDE_BINARY: fake },
  });
  const ended = new Promise(resolve => child.once('close', resolve));
  const stdout = createInterface({ input: child.stdout });
  let stderr = '';
  child.stderr.on('data', chunk => { stderr += chunk; });
  const messages = [];
  try {
    for await (const line of stdout) {
      messages.push(JSON.parse(line));
      child.stdin.end('FIXTURE_BROWSER_ANSWER\n');
    }
    assert.equal(await ended, 0);
    assert.deepEqual(messages, [{
      url: 'https://claude.ai/oauth/authorize?state=FIXTURE_BROWSER_LINK',
      display_code: null, needs_code: true,
    }]);
    assert.equal(stderr, '');
    const credentials = join(shared, '.credentials.json');
    assert.equal(await readFile(credentials, 'utf8'), 'FIXTURE_MACHINE_ONLY_LOGIN');
    assert.equal((await stat(credentials)).mode & 0o777, 0o600);
  } finally {
    child.kill();
    await rm(root, { recursive: true, force: true });
  }
});

test('a vanished parent does not leave the official login running', async () => {
  const root = await mkdtemp(join(tmpdir(), 'calternal-sign-in-gone-'));
  const home = join(root, 'home');
  const shared = join(root, 'login');
  await mkdir(home);
  await mkdir(shared);
  const child = spawn(process.execPath, [launcher, '--sign-in'], {
    env: { HOME: home, PATH: process.env.PATH, CALTERNAL_CLAUDE_LOGIN_DIR: shared,
      CALTERNAL_PROVIDER_LOGIN: '1', CALTERNAL_CLAUDE_BINARY: fake },
  });
  const ended = new Promise(resolve => child.once('close', resolve));
  let pid = 0;
  try {
    for await (const line of createInterface({ input: child.stdout })) {
      JSON.parse(line);
      pid = Number(await readFile(join(home, 'login.pid'), 'utf8'));
      child.stdin.end(); // the machine goes away before any code is relayed
      break;
    }
    assert.ok(pid > 0);
    await Promise.race([ended, new Promise((_, no) => setTimeout(() => no(new Error('adapter outlived its parent')), 5000))]);
    const gone = async () => {
      for (let tries = 0; tries < 50; tries++) {
        try { process.kill(pid, 0); } catch { return true; }
        await new Promise(resolve => setTimeout(resolve, 100));
      }
      return false;
    };
    assert.equal(await gone(), true, 'the login process was left running');
  } finally {
    child.kill();
    if (pid > 0) { try { process.kill(pid, 'SIGKILL'); } catch { /* already gone */ } }
    await rm(root, { recursive: true, force: true });
  }
});
