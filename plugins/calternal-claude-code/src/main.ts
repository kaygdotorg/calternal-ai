/** Stdio entry point; the official Claude Code executable must be installed in the sandbox. */
import { main } from './adapter.js';
import { linkSharedLogin, signIn } from './sign-in.js';

if (process.argv[2] === '--sign-in') {
  try {
    await signIn();
  } catch {
    process.exitCode = 1;
  }
} else {
  await linkSharedLogin();
  await main();
}
