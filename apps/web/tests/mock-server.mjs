// TEST-ONLY mock of the frozen /api/v1 contract so the UI can be exercised before/without the Rust server.
// Never shipped; the real verification runs against the real server. Usage: node tests/mock-server.mjs [port]
import http from 'node:http';
import { randomUUID } from 'node:crypto';
const TOKENS = { 'owner-mock-token': { id: 'owner', spaces: [{ id: 'personal', role: 'manager' }, { id: 'shared', role: 'manager' }] },
  'guest-mock-token': { id: 'guest', spaces: [{ id: 'shared', role: 'reader' }] } };
const notes = new Map(); const seen = new Map(); const enabled = { 'notes-insights': false };
const mk = (space, title, markdown) => { const n = { id: randomUUID(), space_id: space, title, markdown, revision: randomUUID(), updated_at: new Date().toISOString() }; notes.set(n.id, { n, versions: [{ title, markdown, revision: n.revision, updated_at: n.updated_at }] }); return n; };
mk('personal', 'Welcome', '# Hello\nDisposable data.'); mk('shared', 'Shared plan', 'one two three\nfour');
const view = (id, text) => ({ genui_version: 1, root: { type: 'stack', id: 'r' + id, children: [{ type: 'text', id: 't' + id, text }] } });
const manifest = (id) => id === 'notes' ? { id, version: '1.0.0', api_version: 1, name: 'Notes', description: 'Core notes app.', dependencies: [], capabilities: [], operations: [], views: [] }
  : { id, version: '1.0.0', api_version: 1, name: 'Notes Insights', description: 'Word, line and character statistics for a note.', dependencies: [{ id: 'notes', version: '1.0.0' }], capabilities: ['notes.read'],
    operations: [{ id: 'notes-insights.summarize', description: 'Summarize', mutates: false }],
    views: [{ id: 'panel', extension_point: 'notes.detail', view: { genui_version: 1, root: { type: 'stack', id: 'p', children: [{ type: 'text', id: 'pt', text: 'Statistics for this note.' }, { type: 'button', id: 'pb', label: 'Run summarize', action: { operation: 'notes-insights.summarize', input: {} } }, { type: 'button', id: 'evil', label: 'Delete everything', action: { operation: 'notes.update', input: {} } }, { type: 'future-widget', id: 'fw' }] } } }] };
const plug = (id) => ({ manifest: manifest(id), enabled: id === 'notes' || enabled[id], generation: 1 });
const send = (res, status, body) => { res.writeHead(status, { 'content-type': 'application/json' }); res.end(JSON.stringify(body)); };
const err = (res, status, rid, code, message) => send(res, status, { request_id: rid, ok: false, error: { code, message } });
http.createServer((req, res) => {
  const p = TOKENS[(req.headers.authorization || '').replace('Bearer ', '')];
  if (!p) return err(res, 401, null, 'unauthorized', 'bad token');
  if (req.url === '/api/v1/describe') return send(res, 200, { ok: true, data: { protocol_version: 1, genui_version: 1, principal: p, operations: [] } });
  let b = ''; req.on('data', (c) => (b += c)); req.on('end', () => {
    const { request_id: rid, operation: op, input: i } = JSON.parse(b);
    const role = p.spaces.find((s) => s.id === i.space_id)?.role;
    if (!role) return err(res, 403, rid, 'forbidden', 'no access');
    if (seen.has(rid)) return send(res, 200, seen.get(rid));
    const ok = (data, code = 200) => { const body = { request_id: rid, ok: true, data }; if (/create|update/.test(op)) seen.set(rid, body); send(res, code, body); };
    const rec = i.id && notes.get(i.id); const sm = ({ markdown, ...s }) => s;
    if (op === 'notes.list') return ok({ notes: [...notes.values()].map((x) => x.n).filter((n) => n.space_id === i.space_id).map(sm) });
    if (op === 'notes.get') return rec && rec.n.space_id === i.space_id ? ok({ note: rec.n }) : err(res, 404, rid, 'not_found', 'no such note');
    if (op === 'notes.create') return ok({ note: mk(i.space_id, i.title, i.markdown) });
    if (op === 'notes.update') { if (!rec) return err(res, 404, rid, 'not_found', 'no such note'); if (rec.n.revision !== i.base_revision) return err(res, 409, rid, 'conflict', 'stale');
      Object.assign(rec.n, { title: i.title ?? rec.n.title, markdown: i.markdown, revision: randomUUID(), updated_at: new Date().toISOString() }); rec.versions.unshift({ title: rec.n.title, markdown: rec.n.markdown, revision: rec.n.revision, updated_at: rec.n.updated_at }); return ok({ note: rec.n }); }
    if (op === 'notes.history') return ok({ versions: rec.versions, next_cursor: null });
    if (op === 'plugins.list') return ok({ plugins: ['notes', 'notes-insights'].map(plug) });
    if (op === 'plugins.enable' || op === 'plugins.disable') { if (role !== 'manager') return err(res, 403, rid, 'forbidden', 'manager only'); enabled[i.plugin_id] = op.endsWith('enable'); return ok({ plugin: plug(i.plugin_id) }); }
    if (op === 'plugins.preview') return ok({ view: view(i.plugin_id, 'Fixture preview text for ' + i.plugin_id), preview: true });
    if (op === 'notes-insights.summarize') { if (!enabled['notes-insights']) return err(res, 403, rid, 'forbidden', 'plugin disabled'); const t = rec.n.markdown; const w = t.split(/\s+/).filter(Boolean).length;
      return ok({ stats: { words: w, characters: t.length, lines: t.split('\n').length }, view: { genui_version: 1, root: { type: 'stack', id: 's', children: [{ type: 'stat', id: 'w', label: 'Words', value: String(w) }, { type: 'stat', id: 'c', label: 'Characters', value: String(t.length) }] } } }); }
    return err(res, 400, rid, 'invalid_input', 'unknown operation');
  });
}).listen(+process.argv[2] || 3299, '127.0.0.1', () => console.log('mock on', process.argv[2] || 3299));
