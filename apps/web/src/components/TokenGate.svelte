<script lang="ts">
  import { app } from '../lib/store.svelte';
  let token = $state('');
  let input: HTMLInputElement | undefined = $state();
  $effect(() => { input?.focus(); });
  async function submit(e: SubmitEvent) {
    e.preventDefault();
    const t = token;
    token = ''; // never keep the secret in the DOM after submit
    await app.connect(t);
  }
</script>

<main class="gate">
  <form class="panel" onsubmit={submit} autocomplete="off">
    <div class="logo" aria-hidden="true">C</div>
    <h1>Calternal development preview</h1>
    <p class="muted">Localhost engineering preview with disposable data and trusted plugins. It is not production software and has no AI conversations yet.</p>
    <div class="field">
      <label for="tok">Development token</label>
      <input id="tok" bind:this={input} class="input" type="password" bind:value={token} autocomplete="off" autocapitalize="off" spellcheck="false" required aria-describedby="tok-help" aria-invalid={app.connError ? 'true' : undefined} />
      <span id="tok-help" class="muted small">Held in memory for this tab only. It is never written to the URL, storage or cookies, and is forgotten on reload.</span>
    </div>
    {#if app.connError}<div class="banner err" role="alert">{app.connError}</div>{/if}
    <button class="btn primary" type="submit" disabled={app.conn === 'connecting' || !token}>{app.conn === 'connecting' ? 'Connecting…' : 'Connect'}</button>
    <p class="muted small">The token file path is printed by the server at startup; read it from a terminal and paste the value here.</p>
  </form>
</main>

<style>
  .gate { min-height: 100%; display: grid; place-items: center; padding: 20px; }
  .panel { width: min(440px, 100%); display: grid; gap: 14px; padding: 28px; background: var(--card); border: 1px solid var(--line); border-radius: 20px; box-shadow: var(--shadow); }
  .logo { width: 40px; height: 40px; border-radius: 12px; display: grid; place-items: center; background: var(--accent); color: var(--accent-ink); font-weight: 700; font-size: 20px; }
  h1 { font-size: 20px; }
  p { margin: 0; }
  .small { font-size: 12px; }
</style>
