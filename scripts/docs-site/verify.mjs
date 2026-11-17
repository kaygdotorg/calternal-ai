/** Verify generated page parity and local navigation against the static output. */
import { readFile, stat } from 'node:fs/promises';
import path from 'node:path';
import { content, site, base, files, scan } from './common.mjs';
const dist = path.join(site, 'dist');
const origin = 'https://docs.invalid';
const errors = [];
let count = 0;
for (const filename of await files(content)) {
  const relative = path.relative(content, filename);
  const id = relative.replace(/\.md$/, '').replace(/(?:^|\/)index$/, '');
  const canonical = `${base}${id}${id ? '/' : ''}`;
  const source = await readFile(filename, 'utf8');
  for (const twin of [path.join(dist, id, '.md'), path.join(dist, id ? id + '.md' : 'index.md')]) {
    if (await readFile(twin, 'utf8').catch(() => null) !== source) errors.push(`Markdown twin missing/stale: ${twin}`);
  }
  if (!await stat(path.join(dist, id, 'index.html')).catch(() => null)) errors.push(`HTML missing: ${id}`);
  for (const match of source.matchAll(/\]\(([^\s)]+)\)/g)) {
    let url;
    try { url = new URL(match[1], origin + canonical); } catch { continue; }
    if (url.origin !== origin || !url.pathname.startsWith(base)) continue;
    let target = path.join(dist, decodeURIComponent(url.pathname.slice(base.length)));
    const info = await stat(target).catch(() => null);
    if (info?.isDirectory()) target = path.join(target, 'index.html');
    if (!await stat(target).catch(() => null)) errors.push(`${relative}: missing link ${match[1]}`);
  }
  count++;
}
for (const name of ['llms.txt', 'llms-full.txt']) {
  const value = await readFile(path.join(dist, name), 'utf8');
  if (!value.includes('Calternal') || value.length < 200) errors.push(`${name}: empty or wrong content`);
}
await scan(dist);
if (errors.length) throw new Error(errors.join('\n'));
console.log(`Verified ${count} source pages, Markdown twins, local links, LLM indexes and publication scan.`);
