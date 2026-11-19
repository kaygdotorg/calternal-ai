// Shared login entry point, installed in the trusted sandbox image.
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { main } from './adapter.js';
import { linkSharedLogin, signIn } from './sign-in.js';

const binary = '/usr/local/bin/claude';
const home = await mkdtemp(join(tmpdir(), 'calternal-shared-'));
try {
  process.env.HOME = home;
  process.env.CALTERNAL_CLAUDE_BINARY = binary;
  if (process.argv[2] === '--sign-in') {
    await signIn();
  } else if (process.argv.length === 2 && process.env.CALTERNAL_GATEWAY_TOKEN) {
    const bridge = '/run/calternal-broker/client.mjs';
    const { startGateway } = await import(bridge);
    await writeFile('/home/agent/broker-mcp.json', JSON.stringify({
      mcpServers: {workspace:{command:'/usr/local/bin/node',args:[bridge,'mcp'],
        env:{CALTERNAL_GATEWAY_TOKEN:process.env.CALTERNAL_GATEWAY_TOKEN}}},
    }), {mode:0o600});
    const gateway = await startGateway();
    try {
      process.env.CALTERNAL_GATEWAY_URL = gateway.baseUrl;
      await main(binary, true, true);
    } finally { gateway.close(); }
  } else if (process.argv.length === 2 && process.env.CALTERNAL_CLAUDE_LOGIN_DIR) {
    await linkSharedLogin();
    await main(binary, true);
  } else {
    process.exitCode = 1;
  }
} catch {
  process.exitCode = 1;
} finally {
  await rm(home, { recursive: true, force: true });
}
