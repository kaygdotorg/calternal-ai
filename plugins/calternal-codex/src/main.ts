/** Entrypoint for one sandbox-local Codex turn or sign-in. */
import {main} from './adapter.js';
import {signIn} from './sign-in.js';
import {smoke} from './smoke.js';

if (process.argv[2]?.startsWith('--harness-')) {
  try {
    const { lifecycle } = await import(new URL('./lifecycle.mjs', import.meta.url).href);
    await lifecycle(process.argv.slice(2), smoke);
  } catch { process.exitCode = 1; }
} else if (process.argv[2] === '--sign-in') {
  try { await signIn(); } catch { process.exitCode = 1; }
} else if (process.env.CALTERNAL_GATEWAY_TOKEN) {
  const bridge = '/run/calternal-broker/client.mjs';
  const { startGateway } = await import(bridge);
  const gateway = await startGateway();
  try {
    process.env.CALTERNAL_GATEWAY_URL = gateway.baseUrl;
    await main();
  } finally { gateway.close(); }
} else {
  await main();
}
