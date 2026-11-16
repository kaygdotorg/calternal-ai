import { describe, expect, it, vi, beforeEach } from 'vitest';
import { validateView, checkAction } from '../src/lib/genui';
import { parsePath, hrefFor } from '../src/lib/route';
import * as api from '../src/lib/api';

const v = (root: unknown) => ({ genui_version: 1, root });
const text = (id: string, t = 'x') => ({ type: 'text', id, text: t });

describe('genui validation (client-side defence in depth)', () => {
  it('accepts a valid tree and unknown future nodes', () => {
    expect(validateView(v({ type: 'stack', id: 'r', children: [text('a'), { type: 'future', id: 'f' }] }))).toEqual([]);
  });
  it('rejects duplicate ids, bad ids, wrong version, over-deep and over-large trees', () => {
    expect(validateView(v({ type: 'stack', id: 'r', children: [text('a'), text('a')] })).join()).toMatch(/duplicate/);
    expect(validateView(v(text('bad id!'))).join()).toMatch(/invalid id/);
    expect(validateView({ genui_version: 2, root: text('a') }).join()).toMatch(/genui_version/);
    let n: any = text('leaf');
    for (let i = 0; i < 9; i++) n = { type: 'stack', id: 's' + i, children: [n] };
    expect(validateView(v(n)).join()).toMatch(/deeper/);
    const many = Array.from({ length: 130 }, (_, i) => text('t' + i));
    expect(validateView(v({ type: 'stack', id: 'r', children: many })).join()).toMatch(/nodes/);
    expect(validateView(v(text('big', 'a'.repeat(9000)))).join()).toMatch(/too long/);
    expect(validateView(null)).not.toEqual([]);
  });
  it('rejects malformed buttons and references', () => {
    expect(validateView(v({ type: 'button', id: 'b', label: 'x', action: { operation: 'a', input: [] } })).join()).toMatch(/malformed/);
    expect(validateView(v({ type: 'button', id: 'b', label: 'x'.repeat(300), action: { operation: 'a', input: {} } })).join()).toMatch(/too long/);
    expect(validateView(v({ type: 'resource', id: 'r', resource_id: '../x', space_id: 's', label: 'l' })).join()).toMatch(/malformed/);
  });
  it('restricts plugin buttons to declared read-only operations', () => {
    const policy = { allowed: new Set(['notes-insights.summarize']) };
    expect(checkAction(policy, 'notes-insights.summarize')).toBeNull();
    expect(checkAction(policy, 'notes.update')).toMatch(/Blocked/);
    expect(checkAction({ allowed: null }, 'x')).toMatch(/Blocked/);
    expect(checkAction({ allowed: null, disabledReason: 'preview' }, 'x')).toBe('preview');
  });
});

describe('routes are plain paths, no tokens', () => {
  it('round-trips', () => {
    for (const r of [
      { name: 'notes', space: 'personal', id: 'abc' }, { name: 'notes', space: 's', id: 'abc', view: 'history' },
      { name: 'notes', space: 's', view: 'new' }, { name: 'plugins', space: 's', plugin: 'notes-insights' }, { name: 'activity' }
    ] as const) expect(parsePath(hrefFor(r as any))).toEqual(expect.objectContaining(r));
    expect(hrefFor({ name: 'notes', space: 'a b', id: 'x' })).toBe('/s/a%20b/notes/x');
    expect(parsePath('/nope')).toEqual({ name: 'home' });
  });
});

describe('api client', () => {
  beforeEach(() => { api.setToken(null); vi.restoreAllMocks(); });
  it('refuses to call without a token and never puts it in the URL or body', async () => {
    await expect(api.invoke('notes.list', { space_id: 's' })).rejects.toMatchObject({ code: 'unauthorized' });
    api.setToken('secret-token');
    const f = vi.fn(async () => new Response(JSON.stringify({ request_id: 'r', ok: true, data: { notes: [] } }), { status: 200 }));
    vi.stubGlobal('fetch', f);
    await api.invoke('notes.list', { space_id: 's' }, 'r');
    const [url, init] = f.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe('/api/v1/operations');
    expect(String(init.body)).not.toContain('secret-token');
    expect(init.credentials).toBe('omit');
    expect(JSON.parse(String(init.body))).toEqual({ request_id: 'r', operation: 'notes.list', input: { space_id: 's' } });
  });
  it('maps conflict envelopes', async () => {
    api.setToken('t');
    vi.stubGlobal('fetch', async () => new Response(JSON.stringify({ request_id: 'r', ok: false, error: { code: 'conflict', message: 'stale' } }), { status: 409 }));
    await expect(api.invoke('notes.update', {})).rejects.toMatchObject({ isConflict: true, status: 409 });
  });
  it('accepts a bare DescribeResult', async () => {
    api.setToken('t');
    const d = { protocol_version: 1, genui_version: 1, principal: { id: 'owner', spaces: [] }, operations: [] };
    vi.stubGlobal('fetch', async () => new Response(JSON.stringify(d), { status: 200 }));
    expect((await api.describe()).principal.id).toBe('owner');
  });
});
