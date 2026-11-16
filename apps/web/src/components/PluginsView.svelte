<script lang="ts">
  import { app } from '../lib/store.svelte';
  import { hrefFor } from '../lib/route';
  import GenUiView from './GenUiView.svelte';
  import type { GenUiView as View } from '../lib/types';

  const r = $derived(app.route.name === 'plugins' ? app.route : null);
  const space = $derived(r?.space ?? '');
  const sel = $derived(app.plugins.find((p) => p.manifest.id === r?.plugin) ?? null);
  let previews = $state<Record<string, { view?: View; error?: string; busy?: boolean }>>({});
  let actionError = $state<string | null>(null);

  async function preview(id: string) {
    previews[id] = { busy: true };
    try { previews[id] = { view: await app.previewPlugin(space, id) }; }
    catch (e) { previews[id] = { error: (e as Error).message }; }
  }
  async function toggle(id: string, enable: boolean) { actionError = await app.setPluginEnabled(space, id, enable); }
  // Previews run no actions and have no live side effects.
  const previewPolicy = { allowed: null, disabledReason: 'Preview: actions are disabled and nothing is executed.' };
</script>

<section class="view">
  {#if app.pluginsState === 'loading' && !app.plugins.length}
    <div class="skeleton" style="width:40%;height:24px"></div><div class="skeleton" style="height:120px"></div>
  {:else if app.pluginsState === 'error'}
    <div class="banner err" role="alert">{app.pluginsError} <button class="btn quiet" onclick={() => app.loadPlugins(space, true)}>Retry</button></div>
  {:else if !sel}
    <h1>Plugins</h1>
    <p class="muted">Trusted plugins registered with this server. Only shipped, registered bundles can run; nothing can be uploaded from the browser.</p>
    {#if !app.plugins.length}<div class="empty"><h3>No plugins registered</h3></div>{/if}
    <ul class="cards">
      {#each app.plugins as p (p.manifest.id)}
        <li class="card">
          <div class="top"><h3><a href={hrefFor({ name: 'plugins', space, plugin: p.manifest.id })}>{p.manifest.name}</a></h3>
            <span class="chip {p.enabled ? 'ok' : ''}">{p.enabled ? 'Enabled' : 'Disabled'}</span></div>
          <p class="muted">{p.manifest.description}</p>
        </li>
      {/each}
    </ul>
  {:else}
    {@const p = sel}
    {@const pv = previews[p.manifest.id]}
    <div class="crumb"><a href={hrefFor({ name: 'plugins', space })}>← All plugins</a></div>
    <div class="top">
      <h1>{p.manifest.name}</h1>
      <span class="chip {p.enabled ? 'ok' : ''}">{p.enabled ? 'Enabled' : 'Disabled'}</span>
    </div>
    <p class="muted">{p.manifest.description}</p>
    <dl>
      <dt>ID</dt><dd><code>{p.manifest.id}</code> v{p.manifest.version}</dd>
      <dt>Capabilities</dt><dd>{p.manifest.capabilities.length ? p.manifest.capabilities.join(', ') : 'none'}</dd>
      <dt>Depends on</dt><dd>{p.manifest.dependencies.length ? p.manifest.dependencies.map((d) => `${d.id} ${d.version}`).join(', ') : 'nothing'}</dd>
      <dt>Operations</dt><dd>{p.manifest.operations.length ? p.manifest.operations.map((o) => o.id).join(', ') : 'none (host owns Notes methods)'}</dd>
      <dt>Generation</dt><dd>{p.generation}</dd>
    </dl>
    <div class="row">
      {#if p.enabled}
        <button class="btn" disabled={!app.canManage || app.pluginBusy === p.manifest.id} onclick={() => toggle(p.manifest.id, false)}>Disable</button>
      {:else}
        <button class="btn primary" disabled={!app.canManage || app.pluginBusy === p.manifest.id} onclick={() => toggle(p.manifest.id, true)}>Enable</button>
      {/if}
      <button class="btn" disabled={pv?.busy} onclick={() => preview(p.manifest.id)}>{pv?.busy ? 'Loading preview…' : 'Preview'}</button>
      {#if !app.canManage}<span class="muted small">Enable/disable needs the manager role in this space.</span>{/if}
    </div>
    {#if actionError}<div class="banner err" role="alert">{actionError}</div>{/if}
    {#if pv?.error}<div class="banner err" role="alert">{pv.error}</div>{/if}
    {#if pv?.view}
      <section class="preview" aria-label="Preview">
        <div class="ph"><span class="chip warn">Preview</span><span class="muted small">Fixed fixture content. Works while disabled; no worker is started and no data is touched.</span></div>
        <GenUiView view={pv.view} policy={previewPolicy} />
      </section>
    {/if}
    {#if p.manifest.views.length}
      <h2>Contributed views</h2>
      {#each p.manifest.views as v (v.id)}
        <div class="card"><div class="muted small">{v.id} → <code>{v.extension_point}</code></div>
          <GenUiView view={v.view} policy={{ allowed: null, disabledReason: 'Shown for inspection. Buttons run inside a note.' }} /></div>
      {/each}
    {/if}
  {/if}
</section>

<style>
  .view { display: grid; gap: 12px; align-content: start; }
  h1 { font-size: 22px; } h2 { font-size: 15px; margin-top: 8px; } p { margin: 0; }
  .top { display: flex; gap: 10px; align-items: center; justify-content: space-between; }
  .cards { list-style: none; margin: 0; padding: 0; display: grid; gap: 10px; }
  .card { display: grid; gap: 6px; padding: 14px; border: 1px solid var(--line); border-radius: 14px; background: var(--card-2); }
  .card h3 a { color: var(--text); text-decoration: none; } .card h3 a:hover { text-decoration: underline; }
  dl { display: grid; grid-template-columns: max-content 1fr; gap: 6px 16px; margin: 4px 0; }
  dt { color: var(--text-2); } dd { margin: 0; overflow-wrap: anywhere; }
  .row { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .preview { display: grid; gap: 10px; padding: 14px; border: 1.5px dashed var(--line-strong); border-radius: 14px; }
  .ph { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .small { font-size: 12px; } .crumb a { text-decoration: none; }
  code { font-family: var(--mono); }
</style>
