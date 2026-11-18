/** Common update, version and scratch smoke commands for sandbox harness adapters. */
import {execFile} from 'node:child_process';
import {mkdtemp, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {promisify} from 'node:util';

const run = promisify(execFile);

/** Handle an adapter lifecycle command when one is requested. */
export async function lifecycle(args, smoke) {
  const command = args[0];
  if (!['--harness-version', '--harness-update', '--harness-smoke-test'].includes(command)) return false;
  const descriptor = JSON.parse(await readFile(new URL('runtime.json', import.meta.url), 'utf8'));
  if (command === '--harness-update') {
    process.stdout.write(JSON.stringify({
      interface: 1, version: descriptor.version, source: descriptor.source,
      login_command: descriptor.login_command, login_locations: descriptor.login_locations,
      interactive: descriptor.interactive, package_sha256: descriptor.package_sha256,
    }) + '\n');
    return true;
  }
  const home = await mkdtemp(join(tmpdir(), 'calternal-harness-version-'));
  let version;
  try {
    const result = await run(descriptor.executable, ['--version'], {
      timeout: 4000, maxBuffer: 4096, env: {PATH: process.env.PATH, HOME: home},
    });
    version = result.stdout.match(/(?:^|\s)([0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9a-z.]+)?)(?:\s|$)/i)?.[1];
  } finally {
    await rm(home, {recursive: true, force: true});
  }
  if (!version) throw new Error('could not identify the official CLI version');
  if (command === '--harness-smoke-test') {
    if (args[1] !== version || descriptor.version !== version || !descriptor.package_sha256) throw new Error('runtime pin mismatch');
    await smoke();
  }
  process.stdout.write(JSON.stringify({interface: 1, version, package_sha256: descriptor.package_sha256, passed: command === '--harness-smoke-test'}) + '\n');
  return true;
}
