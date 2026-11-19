/** Verify pinned npm artifacts and install the official harness version into the image. */
import {readFileSync, writeFileSync, mkdtempSync, rmSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {join} from 'node:path';
import {tmpdir} from 'node:os';

const registry = '--registry=https://registry.npmjs.org';
const sourcePattern = /^@[a-z0-9-]+\/[a-z0-9-]+@[0-9]+\.[0-9]+\.[0-9]+(?:-linux-(?:x64|arm64))?$/;

function verify(source, integrity, cwd) {
  if (!sourcePattern.test(source) || !/^sha512-[A-Za-z0-9+/]{86}==$/.test(integrity)) {
    throw new Error('invalid official harness artifact pin');
  }
  const [{filename}] = JSON.parse(execFileSync('npm', ['pack', '--json', registry, source], {
    cwd, encoding: 'utf8', timeout: 120_000, maxBuffer: 4 * 1024 * 1024,
  }));
  const bytes = readFileSync(join(cwd, filename));
  if ('sha512-' + createHash('sha512').update(bytes).digest('base64') !== integrity) {
    throw new Error('upstream harness artifact integrity mismatch');
  }
  return createHash('sha256').update(bytes).digest('hex');
}

for (const file of process.argv.slice(2)) {
  const descriptor = JSON.parse(readFileSync(file, 'utf8'));
  const {source, version, integrity, executable} = descriptor;
  const native = descriptor.linux_artifacts[process.arch];
  if (!native || source.split('@').at(-1) !== version || !/^\/usr\/local\/bin\/[a-z0-9-]+$/.test(executable)) {
    throw new Error('invalid harness package, executable or architecture');
  }
  const cwd = mkdtempSync(join(tmpdir(), 'calternal-cli-artifact-'));
  try {
    verify(source, integrity, cwd);
    const package_sha256 = verify(native.source, native.integrity, cwd);
    execFileSync('npm', ['install', '--global', '--prefer-offline', '--no-audit', '--no-fund', registry, source], {
      cwd, stdio: 'inherit', timeout: 120_000,
    });
    const actual = execFileSync(executable, ['--version'], {encoding: 'utf8', timeout: 5000, maxBuffer: 4096});
    if (!actual.split(/\s+/).includes(version)) throw new Error('installed CLI version mismatch');
    writeFileSync(file, JSON.stringify({...descriptor, package_sha256}) + '\n');
    process.stdout.write(actual);
  } finally {
    rmSync(cwd, {recursive: true, force: true});
  }
}
