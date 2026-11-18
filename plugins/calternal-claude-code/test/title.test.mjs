import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

async function title(t, managed = false) {
  const home = await mkdtemp(join(tmpdir(), 'calternal-title-'));
  t.after(() => rm(home, { recursive: true, force: true }));
  await writeFile(join(home, 'signed-in'), '');
  await writeFile(join(home, 'subscription'), '');
  if (managed) await writeFile(join(home, 'managed'), '');
  const child = spawn(process.execPath, ['dist/main.js', '--title'], {
    env: { ...process.env, HOME: home, CALTERNAL_CLAUDE_BINARY: resolve('test/fake-claude.mjs') },
  });
  child.stdin.end('Person: Choose plants for a shaded balcony.\nAssistant: Try parsley.\n');
  const output = [];
  for await (const chunk of child.stdout) output.push(chunk);
  return JSON.parse(Buffer.concat(output).toString());
}

test('the title adapter invokes Haiku with the proven flags and emits only the title', async t => {
  assert.deepEqual(await title(t), { title: 'Three-Pot Balcony Garden' });
});

test('a managed subscription returns only its readiness reason', async t => {
  assert.deepEqual(await title(t, true), { reason: 'managed_policy' });
});
