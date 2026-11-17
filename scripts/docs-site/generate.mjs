/** Derive public Markdown from an allowlist of repository contracts and comments. */
import { readFile, writeFile, mkdir, mkdtemp, rm } from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';
import { root, site, content, base, files, validate } from './common.mjs';

const require = createRequire(path.join(site, 'package.json'));
const { Application, TSConfigReader } = await import(require.resolve('typedoc'));
const check = process.argv.includes('--check');
const pages = new Map();
const sourceReads = new Map();
// Cache each allowlisted text source once for a consistent generation snapshot.
const read = filename => {
  if (!sourceReads.has(filename)) sourceReads.set(filename, readFile(path.join(root, filename), 'utf8'));
  return sourceReads.get(filename);
};
const json = value => '```json\n' + JSON.stringify(value, null, 2) + '\n```';
const slug = value => value.toLowerCase().replace(/[^a-z0-9-]+/g, '-').replace(/^-|-$/g, '');
const preview = '> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.\n\n';

/** Remove internal citations, local paths and unresolved discussion from public extracts. */
function publicText(text) {
  return text
    .replace(/\[([^\]]+)\]\([^)]*(?:reviews|measurements|open-questions)[^)]*\)/g, '$1')
    .replace(/`?docs\/(?:reviews|measurements)\/[^\s`.)]+(?:\.md|\.json)?`?/g, '(internal evidence)')
    .replace(/\/home\/kayg\/Developer\/calternal/g, 'the reference repository')
    .replace(/^Input:.*\n/gm, '')
    .replace(/\n## Open\n[\s\S]*/, '')
    .replace(/\n## How the work is run\n[\s\S]*/, '');
}

/** Register deterministic frontmatter and source attribution; duplicates fail. */
function page(id, title, sources, body) {
  if (pages.has(id)) throw new Error(`Duplicate page: ${id}`);
  const output = `---\ntitle: ${JSON.stringify(title)}\n---\n\n<!-- Generated from: ${sources.join(', ')}. Rebuild: npm run docs:site -->\n\n${preview}${publicText(body).trim()}\n`;
  validate(output, id);
  pages.set(id, output);
}

/** Extract one named section, ending at the next heading of equal or higher level. */
function section(text, heading) {
  const lines = text.split('\n');
  const start = lines.findIndex(line => /^#{1,6} /.test(line) && line.replace(/^#+ /, '') === heading);
  if (start < 0) throw new Error(`Source section missing: ${heading}`);
  const level = lines[start].match(/^#+/)[0].length;
  let end = start + 1;
  while (end < lines.length && !new RegExp(`^#{1,${level}} `).test(lines[end])) end++;
  return lines.slice(start + 1, end).join('\n');
}

const copies = {
  'dev/plugin-api': ['Plugin API decisions', 'docs/plugin-api.md'],
  'dev/addresses': ['Addresses', 'docs/addresses.md'],
  'dev/agent-instructions': ['Agent instructions', 'docs/agent-instructions.md'],
  'dev/composer': ['Composer design', 'docs/composer.md'],
  'dev/glossary': ['Glossary', 'docs/glossary.md'],
  'admin/cli': ['CLI conventions', 'docs/cli.md'],
};
// Map links only to public pages. All other repository links remain readable labels.
const publicLinks = new Map(Object.entries(copies).map(([id, [, source]]) => [source, id]));
function links(text, source) {
  return text.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (match, label, target) => {
    if (/^(?:https?:|mailto:|#)/.test(target)) return match;
    const [file, anchor] = target.split('#');
    const resolved = path.posix.normalize(path.posix.join(path.posix.dirname(source), file));
    const id = publicLinks.get(resolved);
    return id ? `[${label}](${base}${id}/${anchor ? `#${anchor}` : ''})` : label;
  });
}
for (const [id, [title, source]] of Object.entries(copies)) {
  page(id, title, [source], links((await read(source)).replace(/^# .*\n/, ''), source));
}

const operations = JSON.parse(await read('contract/operations.json'));
page('dev/operations/index', 'Operation catalogue', ['contract/operations.json'],
  'Generated operation schemas. Capability and risk are shown only when the catalogue declares them; absence is not a grant.\n\n' +
  operations.map(op => `- [${op.id}](./${slug(op.id)}/): ${op.description}`).join('\n'));
for (const op of operations) {
  const metadata = Object.fromEntries(Object.entries(op).filter(([key]) => !['input_schema', 'output_schema'].includes(key)));
  page(`dev/operations/${slug(op.id)}`, op.id, ['contract/operations.json'],
    `${op.description}\n\nCapability: ${op.required_capability || op.capability || 'not declared in this catalogue; the host policy remains authoritative'}.\n\n## Declaration\n\n${json(metadata)}\n\n## Input\n\n${json(op.input_schema)}\n\n## Output\n\n${json(op.output_schema)}`);
}
const manifest = JSON.parse(await read('contract/plugin-manifest-v1.schema.json'));
function schemaTable(schema) {
  return '| Field | Required | Type / reference | Description |\n| --- | --- | --- | --- |\n' +
    Object.entries(schema.properties || {}).map(([name, field]) => {
      const type = field.$ref || field.type || field.items?.$ref || (field.allOf ? field.allOf.map(x => x.$ref).join(', ') : 'schema');
      return `| \`${name}\` | ${(schema.required || []).includes(name) ? 'yes' : 'no'} | ${Array.isArray(type) ? type.join(', ') : type} | ${(field.description || '').replaceAll('|', '\\|').replaceAll('\n', ' ')} |`;
    }).join('\n');
}
page('dev/manifest', 'Plugin manifest schema', ['contract/plugin-manifest-v1.schema.json'],
  `${manifest.description}\n\n${schemaTable(manifest)}\n\n` +
  Object.entries(manifest.definitions || {}).map(([name, schema]) => `## ${name}\n\n${schema.description || ''}\n\n${schemaTable(schema)}\n\n${json(schema)}`).join('\n\n'));

// Commands and exact output examples come from the same shapes/goldens as CLI tests.
const shapes = JSON.parse(await read('docs/cli-shapes.json'));
const goldenRoots = ['crates/cli/tests/golden', 'crates/machine/tests/golden', 'crates/server/tests/golden/cli'];
const goldens = (await Promise.all(goldenRoots.map(dir => files(path.join(root, dir))))).flat();
const cli = await read('docs/cli.md');
const commandIds = [];
const commands = { ...shapes.commands };
// The prose reference also documents commands not yet registered in cli-shapes.json.
for (const command of ['cl notes create', 'cl notes update', 'cl plugins disable', 'calternal watch', 'cl watch']) {
  if (!(command in commands)) commands[command] = null;
}
for (const [command, shape] of Object.entries(commands)) {
  const id = slug(command);
  commandIds.push(id);
  const key = command.replace(/^(?:cl|calternal) /, '').replaceAll(' ', '-');
  const human = goldens.find(file => path.basename(file) === `${key}.txt`);
  const machine = goldens.find(file => path.basename(file) === `${key}.json`);
  const shortCommand = command.replace(/^(?:cl|calternal) /, '');
  const row = cli.split('\n').find(line => line.startsWith('| `') && line.includes('`' + shortCommand) && !line.includes('---'));
  const sources = ['docs/cli.md', 'docs/cli-shapes.json', ...[human, machine].filter(Boolean).map(file => path.relative(root, file))];
  page(`admin/commands/${id}`, command, sources,
    `${row ? '| Arguments | Purpose | Human output | JSON data |\n| --- | --- | --- | --- |\n' + row : 'Command output shape registered by the CLI tests. See CLI conventions for global options.'}\n\n## JSON data shape\n\n${shape === null ? 'No registered shape in cli-shapes.json; consult the recorded output below and the CLI conventions.' : json(shape)}\n\n## Human example\n\n${human ? 'Recorded test fixture:\n\n```text\n' + (await readFile(human, 'utf8')).trim() + '\n```' : 'No recorded human-output fixture is present for this command. Invocation pattern (supply the required arguments from `--help`):\n\n```sh\n' + command + ' --help\n```'}\n\n## JSON example\n\n${machine ? 'Recorded test fixture:\n\n```json\n' + (await readFile(machine, 'utf8')).trim() + '\n```' : 'No recorded JSON fixture is present. Invocation pattern (supply required arguments):\n\n```sh\n' + command + ' --json\n```'}`);
}
page('admin/commands/index', 'Command reference', ['docs/cli-shapes.json', 'docs/cli.md'], commandIds.map(id => `- [${id}](./${id}/)`).join('\n'));

const glossary = await read('docs/glossary.md');
page('user/spaces-and-roles', 'Spaces and roles', ['docs/glossary.md'], section(glossary, 'People and access'));
page('user/permissions-modes', 'Permission modes', ['docs/agent-instructions.md'], 'Accepted design; the complete approval experience is not a released feature.\n\n' + section(await read('docs/agent-instructions.md'), 'Access'));
page('user/space-instructions', 'Space standing instructions', ['docs/agent-instructions.md'], section(await read('docs/agent-instructions.md'), 'Space standing instructions'));
page('user/threads', 'Threads', ['docs/orchestrator-proposal.md'], 'Accepted orchestrator decisions; this is not a claim that the complete thread experience is implemented. Later decisions below take precedence over earlier proposal text.\n\n' + section(await read('docs/orchestrator-proposal.md'), 'Decided in the grill (2026-11-17)'));
for (const [id, title, source, note] of [
  ['machines', 'Machines', 'crates/machine/README.md', 'Implementation reference. The later orchestrator decision requires Resume after interruption; automatic re-placement described in this README is not the accepted product behavior.'],
  ['secrets', 'Secrets', 'crates/secrets/README.md', 'Per-owner key storage and recovery reference. Operator backup retention and key backup handling require a production runbook.'],
  ['recovery', 'Recovery and backups', 'crates/recovery/PROTOCOL.md', 'Recovery protocol reference, not a complete backup or restore runbook. A verified operator backup procedure is not yet present in the source material.'],
]) page(`admin/${id}`, title, [source], `${note}\n\n${links((await read(source)).replace(/^# .*\n/, ''), source)}`);

const rootsSource = 'crates/roots/src/lib.rs';
const rootsDocs = (await read(rootsSource)).split('\n').filter(line => line.startsWith('//!')).map(line => line.replace(/^\/\/! ?/, '')).join('\n');
page('admin/storage', 'Storage layout', [rootsSource, 'docs/glossary.md', 'crates/content/README.md', 'crates/server/README.md'],
  `${section(glossary, 'Storage')}\n\n## Root guard\n\n${rootsDocs}\n\n## Content storage\n\n${links(await read('crates/content/README.md'), 'crates/content/README.md')}\n\n## Preview durability\n\n${section(await read('crates/server/README.md'), 'Durability profile (preview)')}`);

// Stable rustdoc JSON is unavailable on the pinned stable toolchain; use cargo doc HTML.
const crates = (await files(path.join(root, 'crates'))).filter(file => /\/src\/lib\.rs$/.test(file));
const cratePages = [];
for (const filename of crates) {
  const directory = path.dirname(path.dirname(filename));
  const toml = await readFile(path.join(directory, 'Cargo.toml'), 'utf8');
  const name = toml.match(/^name\s*=\s*"([^"]+)"/m)?.[1];
  if (!name) throw new Error(`Missing crate name: ${directory}`);
  const code = await readFile(filename, 'utf8');
  const included = code.match(/#!\[doc = include_str!\("([^"]+)"\)\]/);
  const docs = included ? await readFile(path.resolve(path.dirname(filename), included[1]), 'utf8') : code.split('\n').filter(line => line.startsWith('//!')).map(line => line.replace(/^\/\/\/! ?|^\/\/! ?/, '')).join('\n');
  const source = path.relative(root, filename);
  const sources = [source, ...(included ? [path.relative(root, path.resolve(path.dirname(filename), included[1]))] : [])];
  page(`dev/crates/${name}`, name, sources,
    `${links(docs, source)}\n\n[Full Rust API reference](${base}dev/rust/${name.replaceAll('-', '_')}/index.html). Generated with stable \`cargo doc --workspace --no-deps\`; dependency documentation is omitted.`);
  cratePages.push(`- [${name}](${base}dev/crates/${name}/)`);
}
page('dev/crates/index', 'Rust crates', ['Cargo.toml', ...crates.map(file => path.relative(root, file))], cratePages.join('\n'));

// TypeDoc's Markdown renderer supplies actual exported TypeScript API documentation.
await mkdir(path.join(site, '.cache'), { recursive: true });
const scratch = await mkdtemp(path.join(site, '.cache/typedoc-'));
for (const pkg of ['plugin-sdk', 'genui']) {
  const directory = path.join(root, 'packages', pkg);
  const output = path.join(scratch, pkg);
  const entryPoints = (await files(path.join(directory, 'src'))).filter(file => file.endsWith('.ts'));
  const app = await Application.bootstrapWithPlugins({
    entryPoints, tsconfig: path.join(directory, 'tsconfig.json'),
    plugin: [require.resolve('typedoc-plugin-markdown')],
    name: `@calternal/${pkg}`, readme: 'none', disableSources: true,
    outputs: [{ name: 'markdown', path: output }],
    excludeExternals: true, excludePrivate: true, excludeProtected: true,
    parametersFormat: 'table', propertiesFormat: 'table',
  }, [new TSConfigReader()]);
  const project = await app.convert();
  if (!project || app.logger.hasErrors()) throw new Error(`TypeDoc conversion failed: ${pkg}`);
  await app.generateOutputs(project);
  if (app.logger.hasErrors()) throw new Error(`TypeDoc rendering failed: ${pkg}`);
  const markdownFiles = (await files(output)).filter(file => file.endsWith('.md'));
  if (!markdownFiles.length) throw new Error(`TypeDoc emitted no Markdown: ${pkg}`);
  for (const filename of markdownFiles) {
    const relative = path.relative(output, filename).replace(/\.md$/, '');
    const apiId = value => `dev/typescript/${pkg}/${value.split('/').map(part => part === 'README' ? 'index' : part === 'index' ? 'module-index' : part.toLowerCase()).join('/')}`;
    const id = apiId(relative);
    let body = await readFile(filename, 'utf8');
    body = body.replace(/\]\(([^)]+)\.md(#[^)]+)?\)/g, (_, target, anchor = '') => {
      const destination = apiId(path.posix.normalize(path.posix.join(path.posix.dirname(relative), target))).replace(/\/index$/, '');
      return `](${base}${destination}/${anchor})`;
    });
    page(id, `${pkg}: ${relative}`, entryPoints.map(file => path.relative(root, file)), body);
  }
}
await rm(scratch, { recursive: true, force: true });
for (const [id, title, body] of [
  ['user/index', 'Using Calternal', '- [Spaces and roles](./spaces-and-roles/)\n- [Threads](./threads/)\n- [Permission modes](./permissions-modes/)'],
  ['admin/index', 'Administration', '- [CLI conventions](./cli/)\n- [Commands](./commands/)\n- [Storage](./storage/)\n- [Machines](./machines/)\n- [Secrets](./secrets/)\n- [Recovery and backups](./recovery/)'],
  ['dev/index', 'Developer reference', '- [Operations](./operations/)\n- [Manifest schema](./manifest/)\n- [Plugin SDK](./typescript/plugin-sdk/)\n- [GenUI API](./typescript/genui/)\n- [Rust crates](./crates/)\n- [Plugin decisions](./plugin-api/)\n- [Addresses](./addresses/)\n- [Agent instructions](./agent-instructions/)\n- [Composer](./composer/)\n- [Glossary](./glossary/)'],
  ['index', 'Calternal documentation', `[User guide](${base}user/) · [Administration](${base}admin/) · [Developer reference](${base}dev/)\n\nGenerated from repository contracts, command fixtures and API comments. Each page includes a source-file comment. Append \`.md\` to a page address for its Markdown source (both \`page.md\` and \`page/.md\` are emitted).\n\n[LLM index](${base}llms.txt) · [Complete Markdown documentation](${base}llms-full.txt)`],
]) page(id, title, ['scripts/docs-site/generate.mjs'], body);

// Compare the entire tree: additions, removals and content drift all fail check mode.
const expected = new Set([...pages.keys()].map(id => path.join(content, id + '.md')));
const existing = await files(content);
const stale = existing.filter(file => !expected.has(file));
for (const [id, text] of pages) {
  const filename = path.join(content, id + '.md');
  if (check) {
    if (await readFile(filename, 'utf8').catch(() => null) !== text) stale.push(filename);
  } else {
    await mkdir(path.dirname(filename), { recursive: true });
    await writeFile(filename, text);
  }
}
if (check && stale.length) throw new Error(`Stale generated documentation (${stale.length} files). Run npm run docs:site.`);
if (!check) for (const filename of stale) await rm(filename);
console.log(`${check ? 'Checked' : 'Generated'} ${pages.size} documentation pages.`);
