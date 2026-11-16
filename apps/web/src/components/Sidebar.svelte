<script lang="ts">
  import { app } from '../lib/store.svelte';
  import { hrefFor } from '../lib/route';
  import { tick } from 'svelte';

  let appMenu = $state(false);
  let filter = $state('');
  let menuBtn: HTMLButtonElement | undefined = $state();

  const current = $derived(app.route.name === 'plugins' ? 'plugins' : 'notes');
  const space = $derived(app.currentSpace);
  const attention = $derived(app.activity.filter((a) => a.kind === 'warning' || a.kind === 'error').length);
  const shown = $derived(app.notes.filter((n) => n.title.toLowerCase().includes(filter.trim().toLowerCase())));
  const activeId = $derived(app.route.name === 'notes' ? app.route.id : undefined);
  const activePlugin = $derived(app.route.name === 'plugins' ? app.route.plugin : undefined);

  function nav(e: MouseEvent, hash: string) {
    app.navOpen = false;
  }
  $effect(() => {
    if (!appMenu) return;
    const close = (e: MouseEvent) => { const t = e.target as Element; if (!t.closest?.('.heading')) appMenu = false; };
    window.addEventListener('click', close); return () => window.removeEventListener('click', close);
  });
  async function toggleMenu() { appMenu = !appMenu; if (appMenu) { await tick(); (document.querySelector('#app-menu a') as HTMLElement)?.focus(); } }
  function menuKey(e: KeyboardEvent) {
    if (e.key === 'Escape') { appMenu = false; menuBtn?.focus(); }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      const items = [...document.querySelectorAll<HTMLElement>('#app-menu a')];
      const i = items.indexOf(document.activeElement as HTMLElement);
      items[(i + (e.key === 'ArrowDown' ? 1 : items.length - 1)) % items.length]?.focus(); e.preventDefault();
    }
  }
  function switchSpace(id: string) {
    app.stashDraft();
    app.note = null;
    app.go(app.route.name === 'plugins' ? { name: 'plugins', space: id } : { name: 'notes', space: id });
  }
  function fmt(iso: string) { const d = new Date(iso); return isNaN(+d) ? '' : d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' }); }
</script>

<nav class="side" aria-label="Primary">
  <div class="heading">
    <button bind:this={menuBtn} class="appbtn" aria-haspopup="menu" aria-expanded={appMenu} onclick={toggleMenu}>
      <span class="logo" aria-hidden="true">{current === 'notes' ? '✎' : '⬡'}</span>
      <span class="appname">{current === 'notes' ? 'Notes' : 'Plugins'}</span>
      <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><path d="M3 4.5 6 7.5 9 4.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>
    </button>
    {#if appMenu}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div id="app-menu" class="menu" role="menu" tabindex="-1" onkeydown={menuKey}>
        <a role="menuitem" href={hrefFor({ name: 'notes', space: space ?? '' })} onclick={(e) => { nav(e, ''); appMenu = false; }}>✎ Notes</a>
        <a role="menuitem" href={hrefFor({ name: 'plugins', space: space ?? '' })} onclick={(e) => { nav(e, ''); appMenu = false; }}>⬡ Plugins</a>
      </div>
    {/if}
  </div>

  <a class="activity" class:on={app.route.name === 'activity'} href="/activity" aria-current={app.route.name === 'activity' ? 'page' : undefined} onclick={(e) => nav(e, '')}>
    <span>Activity</span>
    {#if attention}<span class="chip warn" aria-label="{attention} items need attention">{attention}</span>{/if}
  </a>

  <div class="appnav">
    {#if current === 'notes' && space}
      <div class="row-head">
        <h2>Notes</h2>
        {#if app.canEdit}
          <a class="btn quiet" href={hrefFor({ name: 'notes', space, view: 'new' })} onclick={(e) => nav(e, '')} aria-label="New note">＋ New</a>
        {/if}
      </div>
      <label class="sr-only" for="note-filter">Filter notes</label>
      <input id="note-filter" class="input" type="search" placeholder="Filter notes" bind:value={filter} />
      <ul class="list" aria-label="Notes">
        {#if app.notesState === 'loading' && !app.notes.length}
          {#each [1, 2, 3] as i (i)}<li><div class="skeleton" style="margin:10px 8px"></div></li>{/each}
        {:else if app.notesState === 'error'}
          <li class="note-msg">Could not load notes. <button class="btn quiet" onclick={() => app.loadNotes(space, true)}>Retry</button></li>
        {:else if !shown.length}
          <li class="note-msg muted">{filter ? 'No notes match.' : 'No notes yet.'}</li>
        {:else}
          {#each shown as n (n.id)}
            <li>
              <a class="item" class:on={activeId === n.id} href={hrefFor({ name: 'notes', space, id: n.id })} aria-current={activeId === n.id ? 'page' : undefined} onclick={(e) => nav(e, '')}>
                <span class="t">{n.title}</span>{#if app.hasDraft(space, n.id) || (app.dirty && app.note?.id === n.id)}<span class="chip warn" title="Unsaved draft kept in memory">draft</span>{/if}<span class="d">{fmt(n.updated_at)}</span>
              </a>
            </li>
          {/each}
        {/if}
      </ul>
    {:else if current === 'plugins' && space}
      <div class="row-head"><h2>Plugins</h2></div>
      <ul class="list" aria-label="Plugins">
        {#if app.pluginsState === 'loading' && !app.plugins.length}
          {#each [1, 2] as i (i)}<li><div class="skeleton" style="margin:10px 8px"></div></li>{/each}
        {:else}
          {#each app.plugins as p (p.manifest.id)}
            <li>
              <a class="item" class:on={activePlugin === p.manifest.id} href={hrefFor({ name: 'plugins', space, plugin: p.manifest.id })} aria-current={activePlugin === p.manifest.id ? 'page' : undefined} onclick={(e) => nav(e, '')}>
                <span class="t">{p.manifest.name}</span>
                <span class="dot" class:ok={p.enabled} title={p.enabled ? 'Enabled' : 'Disabled'}></span><span class="sr-only">{p.enabled ? 'enabled' : 'disabled'}</span>
              </a>
            </li>
          {/each}
        {/if}
      </ul>
    {/if}
  </div>

  <div class="footer">
    <span class="chip warn" title="Disposable data, localhost, trusted plugins only">Development preview</span>
    {#if app.info}
      <div class="field">
        <label for="space-sel">Space</label>
        <select id="space-sel" class="input" value={space} onchange={(e) => switchSpace(e.currentTarget.value)}>
          {#each app.spaces as s (s.id)}<option value={s.id}>{s.id} · {s.role}</option>{/each}
        </select>
      </div>
      <div class="who">
        <span class="muted" title="Signed in principal">{app.info.principal.id}</span>
        <button class="btn quiet" onclick={() => { if (app.confirmLeave()) { app.disconnect();} }}>Disconnect</button>
      </div>
    {/if}
  </div>
</nav>

<style>
  .side { display: flex; flex-direction: column; gap: 10px; height: 100%; padding: 12px 10px 10px; min-width: 0; }
  .heading { position: relative; height: 44px; display: flex; align-items: center; }
  .appbtn { display: flex; align-items: center; gap: 8px; width: 100%; height: 40px; padding: 0 10px; border: 0; border-radius: 12px; background: transparent; cursor: pointer; font-weight: 650; font-size: 15px; }
  .appbtn:hover { background: var(--row-hover); }
  .appbtn svg { margin-left: auto; color: var(--text-3); }
  .logo { display: grid; place-items: center; width: 26px; height: 26px; border-radius: 8px; background: var(--accent); color: var(--accent-ink); font-size: 14px; }
  .menu { position: absolute; z-index: 20; top: 44px; left: 0; right: 0; padding: 6px; border-radius: 12px; background: var(--card); border: 1px solid var(--line); box-shadow: var(--shadow); display: grid; gap: 2px; }
  .menu a { padding: 8px 10px; border-radius: 8px; text-decoration: none; color: var(--text); }
  .menu a:hover, .menu a:focus-visible { background: var(--row-hover); }
  .activity { display: flex; align-items: center; justify-content: space-between; min-height: 56px; padding: 0 14px; border: 1.5px solid var(--line-strong); border-radius: 16px; background: var(--card); color: var(--text); text-decoration: none; font-weight: 600; transition: background .12s var(--ease); }
  .activity:hover { background: var(--card-2); }
  .activity.on { border-color: transparent; background: var(--accent-soft); color: var(--accent); }
  .appnav { flex: 1; min-height: 0; display: flex; flex-direction: column; gap: 8px; padding-top: 6px; }
  .row-head { display: flex; align-items: center; justify-content: space-between; padding: 0 6px; }
  .row-head h2 { font-size: 12px; text-transform: uppercase; letter-spacing: .06em; color: var(--text-3); }
  .row-head .btn { text-decoration: none; min-height: 28px; }
  .list { list-style: none; margin: 0; padding: 0; overflow: auto; flex: 1; display: grid; gap: 1px; align-content: start; }
  .item { display: flex; align-items: center; gap: 8px; height: 32px; padding: 0 10px; border-radius: 9px; color: var(--text); text-decoration: none; }
  .item:hover { background: var(--row-hover); }
  .item.on { background: var(--accent-soft); font-weight: 600; }
  .t { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .d { color: var(--text-3); font-size: 12px; }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--line-strong); }
  .dot.ok { background: var(--ok); }
  .note-msg { padding: 10px; font-size: 13px; }
  .footer { display: grid; gap: 8px; padding: 10px 6px 2px; border-top: 1px solid var(--line); }
  .footer .chip { justify-self: start; }
  .who { display: flex; align-items: center; justify-content: space-between; font-size: 12px; gap: 6px; }
  .who .muted { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
