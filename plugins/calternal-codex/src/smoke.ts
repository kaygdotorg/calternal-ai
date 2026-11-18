/** Smoke both nested policies against a disposable workspace and a credential canary. */
import {unlink, writeFile} from 'node:fs/promises';
import {join} from 'node:path';
import {CodexRpc} from './rpc.js';
import {prepareProfile} from './profile.js';

/** Verify writes and credential reads in the same app-server execution path as turns. */
export async function smoke(): Promise<void> {
  const cwd = process.cwd();
  const privateDirectory = process.env.CALTERNAL_PROVIDER_LOGIN_DIR;
  if (!privateDirectory) throw new Error('scratch profile is missing');
  const canary = join(privateDirectory, 'smoke-canary');
  const trial = join(cwd, '.calternal-smoke-write');
  await writeFile(canary, 'disposable canary\n', {flag: 'wx', mode: 0o600});
  try {
    for (const mode of ['least_access', 'full_access'] as const) {
      const profile = await prepareProfile(cwd, mode);
      const rpc = new CodexRpc(profile.binary, profile.args, cwd, profile.env, async () => {});
      try {
        await rpc.initialize();
        const result = await rpc.request('command/exec', {
          command: ['/bin/sh', '-c',
            'if /bin/cat "$1" >/dev/null 2>&1; then exit 3; fi; if /bin/touch "$2" 2>/dev/null; then echo writable; else echo read-only; fi',
            'smoke', canary, trial],
          cwd, permissionProfile: profile.permissions, timeoutMs: 3000,
        });
        const expected = mode === 'least_access' ? 'read-only' : 'writable';
        if (result.exitCode !== 0 || result.stdout !== expected + '\n') throw new Error('Codex sandbox policy failed');
      } finally {
        await rpc.close();
      }
    }
  } finally {
    await unlink(canary);
    await unlink(trial).catch(() => {});
  }
}
