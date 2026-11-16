<script lang="ts">
  import { app } from '../lib/store.svelte';
  const t = (iso: string) => new Date(iso).toLocaleTimeString();
</script>

<section class="view">
  <h1>Activity</h1>
  <p class="muted">Events from this browser session only. Cross-space, persistent Activity is not implemented in the preview.</p>
  {#if !app.activity.length}
    <div class="empty"><h3>Nothing yet</h3><p>Saves, conflicts and plugin changes will appear here.</p></div>
  {:else}
    <ol class="feed" aria-live="polite">
      {#each app.activity as a (a.id)}
        <li class={a.kind}>
          <span class="chip {a.kind === 'info' ? '' : a.kind === 'success' ? 'ok' : a.kind === 'warning' ? 'warn' : 'err'}">{a.kind}</span>
          <span class="txt">{#if a.href}<a href={a.href}>{a.text}</a>{:else}{a.text}{/if}</span>
          <time datetime={a.at} class="muted">{t(a.at)}</time>
        </li>
      {/each}
    </ol>
  {/if}
</section>

<style>
  .view { display: grid; gap: 12px; align-content: start; }
  h1 { font-size: 22px; } p { margin: 0; }
  .feed { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; }
  li { display: flex; align-items: center; gap: 10px; padding: 8px 10px; border-radius: 10px; background: var(--card-2); }
  .txt { flex: 1; min-width: 0; overflow-wrap: anywhere; }
  time { font-size: 12px; }
</style>
