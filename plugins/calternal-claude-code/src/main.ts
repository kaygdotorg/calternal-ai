/** Stdio entry point; the official Claude Code executable must be installed in the sandbox. */
import { main } from './adapter.js';
import { linkSharedLogin, signIn } from './sign-in.js';
import { title } from './title.js';

if (process.argv[2] === '--sign-in') {
  try {
    await signIn();
  } catch {
    process.exitCode = 1;
  }
} else if (process.argv[2] === '--title') {
  try {
    await linkSharedLogin();
    process.stdout.write(JSON.stringify(await title()) + '\n');
  } catch {
    process.stdout.write('{"reason":"failed"}\n');
  }
} else {
  await linkSharedLogin();
  await main();
}
