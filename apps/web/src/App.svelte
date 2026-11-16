<script lang="ts">
  import { app } from './lib/store.svelte';
  import Sidebar from './components/Sidebar.svelte';
  import TokenGate from './components/TokenGate.svelte';
  import AiPane from './components/AiPane.svelte';
  import ActivityView from './components/ActivityView.svelte';
  import NoteView from './components/NoteView.svelte';
  import PluginsView from './components/PluginsView.svelte';

  const r = $derived(app.route);
  const denied = $derived((r.name === 'notes' || r.name === 'plugins') && !!app.info && !app.roleIn(r.space));
  const title = $derived(r.name === 'activity' ? 'Activity' : r.name === 'plugins' ? 'Plugins' : r.name === 'notes' ? 'Notes' : 'Calternal');
  let mainEl: HTMLElement | undefined = $state();

  $effect(() => { document.title = `${r.name === 'notes' && app.note ? app.note.title + ' · ' : ''}${title} · Calternal development preview`; });
  // Opening the drawer moves focus into it; the page behind is inert while it is open.
  $effect(() => { if (app.navOpen) document.querySelector<HTMLElement>('.side-col a, .side-col button')?.focus(); });
  // Move focus to the main region on route change (keyboard/screen-reader navigation).
  $effect(() => { void app.route; mainEl?.focus({ preventScroll: true }); });
  $effect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (app.navOpen) { app.navOpen = false; setTimeout(() => document.querySelector<HTMLElement>('.menu-btn')?.focus(), 0); }
      else if (app.aiOpen && (e.target as Element)?.closest?.('.ai-col')) { app.aiOpen = false; document.querySelector<HTMLElement>('.ai-toggle')?.focus(); }
    };
    // The drawer only exists on narrow screens; never leave the page inert after widening.
    const mq = matchMedia('(max-width: 820px)');
    const onMq = () => { if (!mq.matches) app.navOpen = false; };
    mq.addEventListener('change', onMq);
    window.addEventListener('keydown', onKey); return () => { window.removeEventListener('keydown', onKey); mq.removeEventListener('change', onMq); };
  });
</script>

<a class="skip" href="#main-content" onclick={(e) => { e.preventDefault(); mainEl?.focus(); }}>Skip to content</a>

{#if app.conn !== 'connected'}
  <TokenGate />
{:else}
  <div class="shell" class:ai={app.aiOpen} class:nav-open={app.navOpen}>
    <div class="side-col"><Sidebar /></div>
    {#if app.navOpen}<button class="scrim" aria-label="Close navigation" onclick={() => (app.navOpen = false)}></button>{/if}

    <div class="card" role="region" aria-label={title} inert={app.navOpen}>
      <header class="top">
        <button class="btn quiet menu-btn" aria-label="Open navigation" aria-expanded={app.navOpen} onclick={() => (app.navOpen = !app.navOpen)}>☰</button>
        <span class="crumb">{title}</span>
        <span class="chip warn dev" title="Disposable data, localhost, trusted plugins">Dev preview · disposable data</span>
        <button class="btn quiet ai-toggle" aria-pressed={app.aiOpen} onclick={() => (app.aiOpen = !app.aiOpen)}>
          {app.aiOpen ? 'Hide AI' : 'AI'}
        </button>
      </header>
      <main id="main-content" tabindex="-1" bind:this={mainEl}>
        {#if denied}
          <div class="empty" role="alert"><h3>Space not available</h3><p>“{(r as any).space}” does not exist or your principal has no access to it.</p>
            {#if app.spaces[0]}<a class="btn" href={`/s/${encodeURIComponent(app.spaces[0].id)}/notes`}>Go to {app.spaces[0].id}</a>{/if}</div>
        {:else if r.name === 'activity'}<ActivityView />
        {:else if r.name === 'plugins'}<PluginsView />
        {:else if r.name === 'notes'}<NoteView />
        {:else}<div class="empty"><h3>Welcome</h3><p>Pick a space in the sidebar.</p></div>{/if}
      </main>
    </div>

    {#if app.aiOpen}<div class="ai-col"><AiPane /></div>{/if}
  </div>
{/if}

<style>
  .skip { position: absolute; left: 8px; top: -50px; z-index: 100; padding: 8px 12px; background: var(--card); border-radius: 8px; }
  .skip:focus { top: 8px; }
  .shell { height: 100%; display: grid; grid-template-columns: 272px minmax(0, 1fr); background: var(--shell); }
  .shell.ai { grid-template-columns: 272px minmax(0, 1fr) 340px; }
  .side-col { min-width: 0; min-height: 0; height: 100%; overflow: hidden; }
  .card { margin: 8px 8px 8px 0; min-width: 0; min-height: 0; display: flex; flex-direction: column; background: var(--card); border: 1px solid var(--line); border-radius: var(--r); box-shadow: var(--shadow); overflow: hidden; }
  .shell.ai .card { margin-right: 0; }
  .top { display: flex; align-items: center; gap: 10px; height: 52px; padding: 0 14px 0 18px; border-bottom: 1px solid var(--line); flex-shrink: 0; }
  .crumb { font-weight: 650; flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  main { flex: 1; min-height: 0; overflow: auto; padding: 20px 24px 28px; outline: none; }
  .menu-btn { display: none; }
  .ai-col { min-width: 0; height: 100%; overflow: hidden; }
  .scrim { display: none; }

  @media (max-width: 1100px) {
    .shell.ai { grid-template-columns: 272px minmax(0, 1fr); }
    .ai-col { position: fixed; inset: 0 0 0 auto; width: min(360px, 92vw); z-index: 40; background: var(--shell); box-shadow: -12px 0 40px rgba(0, 0, 0, .2); }
  }
  @media (max-width: 820px) {
    .shell, .shell.ai { grid-template-columns: minmax(0, 1fr); }
    .card { margin: 0; border-radius: 0; border: 0; }
    .side-col { position: fixed; inset: 0 auto 0 0; width: min(300px, 86vw); z-index: 50; background: var(--shell); transform: translateX(-102%); transition: transform .2s var(--ease); box-shadow: 12px 0 40px rgba(0, 0, 0, .2); }
    .shell.nav-open .side-col { transform: none; }
    .shell:not(.nav-open) .side-col { visibility: hidden; transition: transform .2s var(--ease), visibility 0s .2s; }
    .scrim { display: block; position: fixed; inset: 0; z-index: 45; border: 0; background: rgba(0, 0, 0, .35); }
    .menu-btn { display: inline-flex; }
    .dev { display: none; }
    main { padding: 16px; }
  }
</style>
