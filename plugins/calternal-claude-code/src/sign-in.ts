/** The official CLI owns the login. Its diagnostics stay in this machine process. */
import { spawn, type ChildProcess } from 'node:child_process';
import { chmod, copyFile, lstat, mkdir, readlink, rename, rm, symlink } from 'node:fs/promises';
import { join } from 'node:path';
import { createInterface } from 'node:readline';
import { randomUUID } from 'node:crypto';

const credentialName = '.credentials.json';
const hosts = new Set(['claude.ai', 'claude.com', 'platform.claude.com', 'console.anthropic.com']);

function paths() {
  const home = process.env.HOME;
  const login = process.env.CALTERNAL_CLAUDE_LOGIN_DIR;
  if (!home || !login) return;
  const config = join(home, '.claude');
  return { config, local: join(config, credentialName), shared: join(login, credentialName) };
}

/** Point a normal Claude process at the login file while keeping its config and history local. */
export async function linkSharedLogin(): Promise<void> {
  const files = paths();
  if (!files) return;
  await mkdir(files.config, { recursive: true, mode: 0o700 });
  const existing = await readlink(files.local).catch(() => undefined);
  if (existing === files.shared) return;
  await rm(files.local, { force: true });
  try {
    await symlink(files.shared, files.local);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'EEXIST' ||
        await readlink(files.local).catch(() => undefined) !== files.shared) throw error;
  }
}

function authorizeUrl(raw: string): string | undefined {
  try {
    const url = new URL(raw);
    if (url.protocol !== 'https:' || !hosts.has(url.hostname) || url.pathname !== '/oauth/authorize' ||
        url.port || url.username || url.password || url.hash || raw.length > 3800) return;
    const redirect = url.searchParams.get('redirect_uri');
    if (redirect) {
      const callback = new URL(redirect);
      if (callback.protocol !== 'https:' || !hosts.has(callback.hostname)) return;
    }
    return raw;
  } catch { return; }
}

function exit(child: ChildProcess): Promise<number | null> {
  return new Promise(resolve => {
    child.once('error', () => resolve(null));
    child.once('close', resolve);
  });
}

async function publish(local: string, shared: string): Promise<void> {
  const info = await lstat(local);
  if (!info.isFile() || info.size > 1024 * 1024) throw new Error('invalid credential file');
  const next = shared + '.' + randomUUID();
  try {
    await copyFile(local, next);
    await chmod(next, 0o600);
    await rename(next, shared);
  } finally {
    await rm(next, { force: true });
  }
}

/** Relay a bounded browser challenge over a private pipe. A clean exit requires the official status check. */
export async function signIn(): Promise<void> {
  const files = paths();
  if (!files || process.env.CALTERNAL_PROVIDER_LOGIN !== '1') throw new Error('private login home required');
  const binary = process.env.CALTERNAL_CLAUDE_BINARY ?? 'claude';
  const env = {
    PATH: process.env.PATH,
    HOME: process.env.HOME,
    LANG: 'C.UTF-8',
    BROWSER: '/bin/false',
  };
  const login = spawn(binary, ['auth', 'login'], { env });
  const ended = exit(login);
  const timer = setTimeout(() => login.kill('SIGKILL'), 9 * 60_000);
  const input = createInterface({ input: process.stdin, terminal: false });
  let output = '';
  let prompted = false;
  const observe = (chunk: Buffer) => {
    if (prompted) return;
    output += chunk.toString('utf8');
    if (output.length > 64 * 1024) { login.kill('SIGKILL'); return; }
    const text = output.replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, '');
    // Wait for a delimiter before validating a possibly split URL.
    for (const candidate of text.matchAll(/https:\/\/[^\s<>"']+(?=\s|[<>"'])/g)) {
      const url = authorizeUrl(candidate[0]);
      if (url) {
        prompted = true;
        output = '';
        process.stdout.write(JSON.stringify({ url, display_code: null, needs_code: true }) + '\n');
        break;
      }
    }
  };
  login.stdout.on('data', observe);
  login.stderr.on('data', observe);
  login.stdin.on('error', () => {});
  input.on('line', code => {
    if (prompted && code.length > 0 && code.length <= 2048 && !/[\x00-\x1f\x7f]/.test(code)) {
      login.stdin.write(code + '\n');
    }
  });
  try {
    if (await ended !== 0) throw new Error('official login failed');
    const status = spawn(binary, ['auth', 'status'], { env, stdio: 'ignore' });
    const check = setTimeout(() => status.kill('SIGKILL'), 15_000);
    try {
      if (await exit(status) !== 0) throw new Error('provider status failed');
    } finally {
      clearTimeout(check);
    }
    try {
      await publish(files.local, files.shared);
    } catch (error) {
      // With the default profile, macOS uses one Keychain entry even though
      // each invocation has a distinct HOME. The CLI verified that entry.
      if (process.platform !== 'darwin' || (error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
    }
  } finally {
    clearTimeout(timer);
    input.close();
    process.stdin.pause();
  }
}
