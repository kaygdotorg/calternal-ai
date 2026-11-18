/** Stdio entry point; the official Claude Code executable must be installed in the sandbox. */
import { main } from './adapter.js';
import { linkSharedLogin, signIn } from './sign-in.js';

if (process.argv[2]?.startsWith('--harness-')) {
  try {
    const { lifecycle } = await import(new URL('./lifecycle.mjs', import.meta.url).href);
    await lifecycle(process.argv.slice(2), async () => {
      const { spawn } = await import('node:child_process');
      const proc = spawn('/usr/local/bin/claude', ['auth', 'status'], {stdio: 'ignore'});
      const timer = setTimeout(() => proc.kill('SIGKILL'), 3000);
      try {
        const code = await new Promise((resolve, reject) => {
          proc.once('error', reject);
          proc.once('exit', resolve);
        });
        // The candidate runs with an empty profile: the official status must
        // report that it is signed out, rather than succeeding with ambient auth.
        if (code !== 1) throw new Error('Claude Code scratch profile failed');
      } finally {
        clearTimeout(timer);
      }
    });
  } catch { process.exitCode = 1; }
} else if (process.argv[2] === '--sign-in') {
  try {
    await signIn();
  } catch {
    process.exitCode = 1;
  }
} else {
  await linkSharedLogin();
  await main();
}
