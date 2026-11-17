/** Build, validate and emit a static site; no server or product state is modified. */
import { spawnSync } from 'node:child_process';
import { readFile, writeFile, mkdir, cp, rm, stat } from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';
import { root, site, content, files, scan } from './common.mjs';

const start = performance.now();
const require = createRequire(path.join(site, 'package.json'));
const check = process.argv.includes('--check');
/** Run a required subprocess and propagate failures immediately. */
function run(command, args, cwd = root) {
  const result = spawnSync(command, args, { cwd, stdio: 'inherit', env: process.env });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} failed (${result.status})`);
}
run('npm', ['ci', '--prefix', 'apps/docs', '--ignore-scripts', '--no-audit', '--no-fund']);
// Workspace builds supply the contract declaration output resolved by TypeDoc.
if (!await stat(path.join(root, 'node_modules/typescript')).catch(() => null)) {
  run('npm', ['ci', '--ignore-scripts', '--no-audit', '--no-fund']);
}
run('npm', ['run', 'build:plugins']);
run('node', ['scripts/docs-site/generate.mjs', ...(check ? ['--check'] : [])]);
if (check) process.exit(0);
// Documentation needs no debug symbols or incremental object cache.
process.env.CARGO_PROFILE_DEV_DEBUG ||= '0';
process.env.CARGO_INCREMENTAL ||= '0';
// Stable Rust still requires -Z unstable-options for JSON; use the allowed HTML fallback.
run('cargo', ['doc', '--locked', '--workspace', '--no-deps', '-j6']);
const rust = path.join(site, 'public/dev/rust');
await rm(rust, { recursive: true, force: true });
await mkdir(path.dirname(rust), { recursive: true });
await cp(path.join(root, process.env.CARGO_TARGET_DIR || 'target', 'doc'), rust, { recursive: true });
// Internal evidence citations are not public documentation; remove them from rustdoc.
for (const filename of await files(rust)) {
  if (filename.endsWith('.html')) {
    const html = await readFile(filename, 'utf8');
    await writeFile(filename, html.replace(/docs\/(?:reviews|measurements)\/[a-zA-Z0-9._/-]+/g, '(internal evidence)'));
  }
}
run('npm', ['run', 'build', '--prefix', 'apps/docs']);
const dist = path.join(site, 'dist');
// Emit source beside canonical HTML, and also support appending .md after its slash.
for (const filename of await files(content)) {
  const relative = path.relative(content, filename);
  const id = relative.replace(/\.md$/, '').replace(/(?:^|\/)index$/, '');
  const markdown = await readFile(filename, 'utf8');
  const canonical = path.join(dist, id, '.md');
  const sibling = id ? path.join(dist, id + '.md') : path.join(dist, 'index.md');
  await mkdir(path.dirname(canonical), { recursive: true });
  await mkdir(path.dirname(sibling), { recursive: true });
  await writeFile(canonical, markdown);
  await writeFile(sibling, markdown);
}
// The Rust HTML fallback also has Markdown twins, converted from rustdoc content.
const TurndownService = require('turndown');
const turndown = new TurndownService({ codeBlockStyle: 'fenced' });
for (const filename of await files(path.join(dist, 'dev/rust'))) {
  if (!filename.endsWith('.html')) continue;
  const html = await readFile(filename, 'utf8');
  const main = html.match(/<main[^>]*>([\s\S]*?)<\/main>/)?.[1] || html;
  const markdown = `<!-- Generated from: cargo doc HTML (${path.relative(dist, filename)}); crate source comments. Rebuild: npm run docs:site -->\n\n${turndown.turndown(main)}\n`;
  await writeFile(filename + '.md', markdown);
  if (path.basename(filename) === 'index.html') await writeFile(path.join(path.dirname(filename), '.md'), markdown);
}
await scan(dist);
run('node', ['scripts/docs-site/verify.mjs']);
console.log(`Documentation built: ${(performance.now() - start).toFixed(0)} ms; ${dist}`);
