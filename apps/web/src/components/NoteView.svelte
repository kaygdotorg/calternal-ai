<script lang="ts">
  import { app } from '../lib/store.svelte';
  import { hrefFor } from '../lib/route';
  import GenUiView from './GenUiView.svelte';
  import type { ActionPolicy } from '../lib/genui';

  const r = $derived(app.route.name === 'notes' ? app.route : null);
  const isNew = $derived(r?.view === 'new');
  const showHistory = $derived(r?.view === 'history');
  const n = $derived(app.note);
  const space = $derived(r?.space ?? '');
  const readOnly = $derived(!app.canEdit);
  const lenBytes = $derived(new TextEncoder().encode(app.draftMarkdown).length);
  const tooBig = $derived(lenBytes > 1024 * 1024);

  const ip = $derived(app.insightsPlugin);
  // Extension buttons may invoke only the declared read-only operations of that plugin.
  const policy = $derived<ActionPolicy>({
    allowed: new Set(ip?.manifest.operations.filter((o) => !o.mutates).map((o) => o.id) ?? [])
  });
  const nativeSummarize = $derived(!ip?.manifest.views.some((v) => JSON.stringify(v.view).includes('notes-insights.summarize')));
  const detailViews = $derived(ip?.manifest.views.filter((v) => v.extension_point === 'notes.detail') ?? []);

  function key(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 's') { e.preventDefault(); isNew ? app.createNote() : app.saveNote(); }
  }
  const fmt = (iso: string) => new Date(iso).toLocaleString();
  const saveDisabled = $derived(app.saving || tooBig || readOnly || (!isNew && !app.dirty));
</script>

<svelte:window onkeydown={key} />

{#if app.noteState === 'loading'}
  <div class="pad" aria-busy="true" aria-label="Loading note"><div class="skeleton" style="width:40%;height:24px"></div><div class="skeleton" style="height:200px;margin-top:16px"></div></div>
{:else if app.noteState === 'error'}
  <div class="empty" role="alert">
    <h3>{app.noteError?.code === 'not_found' ? 'Note not found' : 'Could not open note'}</h3>
    <p>{app.noteError?.message}</p>
    <a class="btn" href={hrefFor({ name: 'notes', space })}>Back to notes</a>
  </div>
{:else if !n && !isNew}
  <div class="empty">
    <h3>Select a note</h3>
    <p>Choose a note from the sidebar{app.canEdit ? ' or create a new one' : ''}.</p>
    {#if app.canEdit}<a class="btn primary" href={hrefFor({ name: 'notes', space, view: 'new' })}>New note</a>{/if}
  </div>
{:else}
  <div class="wrap">
    <h1 class="sr-only">{isNew ? 'New note' : n?.title ?? 'Note'}</h1>
    <div class="sr-only" role="status" aria-live="polite">{app.saving ? 'Saving' : app.dirty || isNew ? 'Unsaved changes' : 'All changes saved'}</div>
    <div class="head">
      <label class="sr-only" for="note-title">Title</label>
      <input id="note-title" class="title" placeholder="Untitled" maxlength="256" bind:value={app.draftTitle} readonly={readOnly} />
      <div class="actions">
        {#if app.dirty || isNew}<span class="chip warn">{isNew ? 'New' : 'Unsaved'}</span>{:else}<span class="chip ok">Saved</span>{/if}
        {#if !isNew && n}
          <a class="btn quiet" href={hrefFor({ name: 'notes', space, id: n.id, view: showHistory ? undefined : 'history' })}>{showHistory ? 'Hide history' : 'History'}</a>
        {/if}
        {#if app.dirty && !isNew && !readOnly}<button class="btn quiet" onclick={() => app.revertDraft()}>Revert</button>{/if}
{#if !readOnly}        <button class="btn primary" disabled={saveDisabled} onclick={() => (isNew ? app.createNote() : app.saveNote())}>
          {app.saving ? 'Saving…' : isNew ? 'Create' : 'Save'}
        </button>{/if}
      </div>
    </div>

    {#if readOnly}<div class="banner">You have read-only access to this space.</div>{/if}
    {#if app.conflict}
      <div class="banner warn conflict" role="alert">
        <div class="cbody">
          <strong>Save conflict.</strong> A newer revision was saved by someone else. Your draft below is untouched.
          <div class="cols">
            <div><div class="lbl">Latest on server · {fmt(app.conflict.updated_at)}</div><pre>{app.conflict.title}{'\n\n'}{app.conflict.markdown}</pre></div>
          </div>
          <div class="cact">
            <button class="btn" onclick={() => app.keepMineOverLatest()}>Keep my draft (save on top of latest)</button>
            <button class="btn danger" onclick={() => { if (confirm('Discard your draft and load the latest revision?')) app.discardDraftForLatest(); }}>Discard draft, load latest</button>
          </div>
        </div>
      </div>
    {/if}
    {#if app.saveError}<div class="banner err" role="alert">{app.saveError} <button class="btn quiet" onclick={() => (isNew ? app.createNote() : app.saveNote())}>Retry</button></div>{/if}
    {#if tooBig}<div class="banner err" role="alert">Document exceeds the 1 MiB preview limit.</div>{/if}

    <label class="sr-only" for="note-body">Markdown</label>
    <textarea id="note-body" class="body" spellcheck="true" placeholder={readOnly ? '' : 'Write markdown…'} bind:value={app.draftMarkdown} readonly={readOnly}></textarea>
    <div class="foot muted">
      {#if n}Revision <code>{n.revision.slice(0, 12)}</code> · updated {fmt(n.updated_at)} ·{/if}
      {lenBytes.toLocaleString()} bytes · plain-text editor proof (no collaborative or rich editing)
    </div>

    {#if showHistory && n}
      <section class="panel" aria-label="Version history">
        <h2>History</h2>
        {#if app.historyState === 'loading' && !app.history.length}<div class="skeleton"></div>
        {:else if app.historyState === 'error'}<div class="banner err">{app.historyError} <button class="btn quiet" onclick={() => app.loadHistory()}>Retry</button></div>
        {:else if !app.history.length}<p class="muted">No versions yet.</p>
        {:else}
          <ol class="hist">
            {#each app.history as v, i (v.revision)}
              <li>
                <div><strong>{v.title}</strong> {#if i === 0}<span class="chip accent">Latest</span>{/if}
                  <div class="muted small">{fmt(v.updated_at)} · {v.revision.slice(0, 10)} · {v.markdown.length.toLocaleString()} chars</div></div>
                {#if app.canEdit && i > 0}<button class="btn quiet" onclick={() => app.restoreVersion(v)}>Load into draft</button>{/if}
              </li>
            {/each}
          </ol>
          {#if app.historyNext}<button class="btn" onclick={() => app.loadHistory(true)} disabled={app.historyState === 'loading'}>Load older</button>{/if}
        {/if}
      </section>
    {/if}

    {#if ip && n && !isNew}
      <section class="panel" aria-label="Extension: {ip.manifest.name}">
        <h2>{ip.manifest.name} <span class="chip">extension · read-only</span></h2>
        {#key n.id + n.revision}
          {#each detailViews as v (v.id)}
            <GenUiView view={v.view} {policy} context={{ space_id: n.space_id, id: n.id }} />
          {/each}
        {/key}
        {#if nativeSummarize}<div class="row">
          <button class="btn" onclick={() => app.summarize()} disabled={app.insightsBusy || app.dirty}>{app.insightsBusy ? 'Calculating…' : 'Summarize statistics'}</button>
          {#if app.dirty}<span class="muted small">Save first: statistics run on the saved revision.</span>{/if}
        </div>{/if}
        {#if app.insightsError}<div class="banner err" role="alert">{app.insightsError}</div>{/if}
        {#if app.insights && app.insights.noteId === n.id}
          <div aria-live="polite">
            <p class="muted small">Computed by the extension worker for revision {app.insights.revision.slice(0, 10)} (word/line/character counts, not AI).</p>
            <GenUiView view={app.insights.result.view} {policy} context={{ space_id: n.space_id, id: n.id }} />
          </div>
        {/if}
      </section>
    {/if}
  </div>
{/if}

<style>
  .pad { padding: 8px; }
  .wrap { display: flex; flex-direction: column; gap: 12px; min-height: 100%; }
  .head { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; }
  .title { flex: 1 1 220px; min-width: 0; border: 0; background: transparent; font-size: 26px; font-weight: 700; letter-spacing: -.02em; padding: 4px 0; border-bottom: 1px solid transparent; }
  .title:hover { border-bottom-color: var(--line-strong); }
  .actions { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .actions a { text-decoration: none; color: var(--text); }
  .body { flex: 1; min-height: 280px; width: 100%; resize: vertical; padding: 14px 16px; border: 1px solid var(--line); border-radius: 12px; background: var(--card-2); font: 14px/1.6 var(--mono); }
  .foot { font-size: 12px; }
  .panel { display: grid; gap: 10px; padding: 14px; border: 1px solid var(--line); border-radius: 14px; background: var(--card-2); }
  .panel h2 { font-size: 14px; display: flex; gap: 8px; align-items: center; }
  .row { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
  .small { font-size: 12px; } p { margin: 0; }
  .hist { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; }
  .hist li { display: flex; justify-content: space-between; gap: 10px; align-items: center; padding: 8px 10px; border-radius: 10px; background: var(--card); border: 1px solid var(--line); }
  .conflict pre { margin: 6px 0 0; max-height: 160px; overflow: auto; padding: 8px; border-radius: 8px; background: var(--card); white-space: pre-wrap; font: 12px/1.5 var(--mono); }
  .cbody { display: grid; gap: 8px; width: 100%; }
  .lbl { font-size: 12px; font-weight: 600; }
  .cact { display: flex; gap: 8px; flex-wrap: wrap; }
  code { font-family: var(--mono); }
</style>
