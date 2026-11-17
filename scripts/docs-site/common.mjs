/** Shared paths and publication validation for the offline documentation builder. */
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

/** Repository root resolved from this module, independent of the caller directory. */
export const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
/** Isolated static documentation application. */
export const site = path.join(root, 'apps/docs');
/** Generated Markdown tree used by Starlight. */
export const content = path.join(site, 'src/content/docs');
/** Slash-normalized publishing prefix used by generated links. */
export const base = `/${(process.env.DOCS_BASE || '/docs/').split('/').filter(Boolean).join('/')}/`.replace('//', '/');

/** List regular files deterministically in O(entries); refuse symlinks and I/O failures. */
export async function files(directory) {
  const result = [];
  for (const entry of await readdir(directory, { withFileTypes: true }).catch(error => {
    if (error.code === 'ENOENT') return [];
    throw error;
  })) {
    const filename = path.join(directory, entry.name);
    if (entry.isDirectory()) result.push(...await files(filename));
    else if (entry.isFile()) result.push(filename);
    else throw new Error(`Refusing nonregular publication input: ${filename}`);
  }
  return result.sort();
}

/** Scan one text value in O(bytes); throw on private identifiers or internal paths. */
export function validate(text, filename) {
  if (/calternal-dev|kayg@|\.vpn\.|docs\/(?:reviews|measurements)\/|docs\/open-questions/i.test(text)) {
    throw new Error(`Private material in ${filename}`);
  }
}

/** Scan output in O(total text bytes), including rustdoc sources; refuse private material. */
export async function scan(directory) {
  for (const filename of await files(directory)) {
    if (/\.(?:html?|md|txt|json|js|mjs|css|svg|xml|map)$/.test(filename)) {
      validate(await readFile(filename, 'utf8'), path.relative(root, filename));
    }
  }
}
