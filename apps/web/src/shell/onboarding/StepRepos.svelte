<script lang="ts">
  // Coding step one: the repositories threads work on. Paste an address (https or ssh) and add it; each added one is listed. The server's repositories operations do not exist yet (the real implementation says so); until they do this
  // step explains where to add one (the first thread) and can be skipped.
  import { Button } from "@calternal/ui";
  import type { OnboardingApi, RepoInfo } from "./api";
  let { api, ondone }: { api: OnboardingApi; ondone: () => void } = $props();
  let repos = $state<RepoInfo[]>([]), url = $state(""), error = $state(""), unsupported = $state(false), busy = $state(false);
  $effect(() => { void api.repositories().then((r) => (repos = r), () => (unsupported = true)); });
  async function add(e: SubmitEvent) {
    e.preventDefault(); busy = true; error = "";
    try { repos = [...repos, await api.addRepository(url.trim())]; url = ""; } catch { unsupported = true; }
    busy = false;
  }
</script>

<div class="in">
  <p class="note">Threads work on repositories. Add the ones you want to start with: an https or ssh address.</p>
  {#if unsupported}
    <p class="note" role="status">Adding repositories from here arrives with the server's repository support. For now, name one in your first thread and the agent will clone it.</p>
  {:else}
    <form onsubmit={add}>
      <label>Repository address<input name="repo" bind:value={url} placeholder="https://git.example.org/you/project.git" autocomplete="off" spellcheck="false" /></label>
      <Button type="submit" variant="secondary" text="Add" disabled={busy || !url.trim()} />
    </form>
    {#if repos.length}<ul>{#each repos as r (r.id)}<li><strong>{r.name}</strong><small>{r.url}</small></li>{/each}</ul>{/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  {/if}
  <div class="go"><Button variant="primary" text="Continue" onclick={ondone} /></div>
</div>

<style>
  .in { display: grid; gap: var(--s-5); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
  form { display: flex; gap: var(--s-4); align-items: flex-end; }
  label { display: grid; gap: var(--s-2); flex: 1; font-size: var(--text-label); color: var(--ink-3); }
  input { border: 0; min-height: var(--pill-h); padding: 0 var(--s-5); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); font-size: var(--text-ui); }
  ul { list-style: none; padding: 0; margin: 0; display: grid; gap: var(--s-3); }
  li { display: grid; padding: var(--s-4) var(--s-5); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.05); }
  li small { color: var(--ink-3); font-size: var(--text-label); overflow-wrap: anywhere; }
  .error { font-size: var(--text-label); color: var(--danger); }
  .go { display: flex; gap: var(--s-4); }
</style>
