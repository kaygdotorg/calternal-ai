import { describe, expect, it } from 'vitest';
import { mount, unmount, flushSync } from 'svelte';
import GenUiView from '../src/components/GenUiView.svelte';

const policy = { allowed: new Set<string>(), disabledReason: 'preview' };
function render(view: unknown) {
  const target = document.createElement('div'); document.body.append(target);
  const c = mount(GenUiView, { target, props: { view, policy } }); flushSync();
  return { target, done: () => unmount(c) };
}

describe('GenUI renderer', () => {
  it('renders text as text, never HTML', () => {
    const { target, done } = render({ genui_version: 1, root: { type: 'stack', id: 'r', children: [
      { type: 'text', id: 't', text: '<img src=x onerror=alert(1)><script>alert(2)</script>' }] } });
    expect(target.querySelector('img,script')).toBeNull();
    expect(target.textContent).toContain('<img src=x');
    done();
  });
  it('shows an explicit fallback for unknown nodes and never dispatches', () => {
    const { target, done } = render({ genui_version: 1, root: { type: 'stack', id: 'r', children: [{ type: 'hologram', id: 'h', action: { operation: 'notes.update', input: {} } }] } });
    expect(target.textContent).toMatch(/Unsupported element/);
    expect(target.querySelector('button')).toBeNull();
    done();
  });
  it('rejects malformed trees without rendering them', () => {
    const { target, done } = render({ genui_version: 1, root: { type: 'stack', id: 'r', children: [{ type: 'text', id: 'a', text: 'x' }, { type: 'text', id: 'a', text: 'y' }] } });
    expect(target.textContent).toMatch(/View rejected/);
    expect(target.textContent).not.toContain('y');
    done();
  });
  it('disables buttons whose action is not an allowed read-only operation', () => {
    const { target, done } = render({ genui_version: 1, root: { type: 'button', id: 'b', label: 'Go', action: { operation: 'notes.update', input: {} } } });
    expect((target.querySelector('button') as HTMLButtonElement).disabled).toBe(true);
    done();
  });
});
