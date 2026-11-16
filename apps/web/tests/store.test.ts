import { describe, expect, it, vi } from 'vitest';
import { app } from '../src/lib/store.svelte';

const describeBody = { protocol_version: 1, genui_version: 1, principal: { id: 'owner', spaces: [{ id: 'personal', role: 'manager' }] }, operations: [] };
const json = (b: unknown, status = 200) => new Response(JSON.stringify(b), { status });

describe('session safety', () => {
  it('drops in-flight results after disconnect and leaks no prior-account data', async () => {
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    vi.stubGlobal('fetch', async (url: string, init?: RequestInit) => {
      if (url.endsWith('/describe')) return json(describeBody);
      const op = JSON.parse(String(init?.body)).operation;
      if (op === 'notes.list') { await gate; return json({ request_id: 'r', ok: true, data: { notes: [{ id: 'n1', space_id: 'personal', title: 'SECRET', revision: 'v1', updated_at: '2026-01-01T00:00:00Z' }] } }); }
      return json({ request_id: 'r', ok: true, data: { plugins: [] } });
    });
    history.replaceState(null, '', '/s/personal/notes');
    await app.connect('tok-a');
    expect(app.conn).toBe('connected');
    const pending = app.loadNotes('personal', true);
    app.drafts['personal/n1'] = { base: null, title: 'd', markdown: 'draft' };
    app.disconnect();
    release();
    await pending;
    expect(app.notes).toEqual([]);
    expect(app.drafts).toEqual({});
    expect(app.info).toBeNull();
    expect(app.conn).toBe('disconnected');
  });
  it('a stale 401 from a previous token does not disconnect the new session', async () => {
    let n = 0;
    let releaseOld!: () => void;
    const old = new Promise<void>((r) => (releaseOld = r));
    vi.stubGlobal('fetch', async (url: string) => {
      if (url.endsWith('/describe')) return json(describeBody);
      if (n++ === 0) { await old; return json({ request_id: 'r', ok: false, error: { code: 'unauthorized', message: 'x' } }, 401); }
      return json({ request_id: 'r', ok: true, data: { notes: [] } });
    });
    await app.connect('tok-1');
    const first = app.loadNotes('personal', true);
    app.disconnect();
    await app.connect('tok-2');
    releaseOld();
    await first;
    expect(app.conn).toBe('connected');
  });
});
