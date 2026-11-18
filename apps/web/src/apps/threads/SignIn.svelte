<script lang="ts">
  // A provider that needs the person to sign in (again), drawn in the conversation and in the request bar (model.ts `SignInRequest`). The server's line reads "<Provider> needs you to sign in again". It shows the link to open and the
  // short code to type there (when the provider has them); when the provider shows a code to paste back (`needsCode`), a field takes it. While it is open the status is asked every few seconds; on `done` the item turns into
  // "Authentication done" and the open requests are read again. Nothing of this is kept in the thread: the thread file carries one line.
  import { onMount } from "svelte";
  import { Button, Icon } from "@calternal/ui";
  import type { SignInRequest } from "./model";
  let { request, result = "pending", onanswer, onpoll }: { request: SignInRequest; result?: "pending" | "done" | "failed"; onanswer: (code: string) => void; onpoll?: () => Promise<"pending" | "done" | "failed" | "gone"> } = $props();
  let code = $state("");
  let sent = $state(false);
  let ended = $state<"done" | "failed" | null>(null);
  const shown = $derived(ended ?? (result === "pending" ? null : result));
  onMount(() => {
    if (!onpoll) return;
    const t = setInterval(() => { void onpoll().then((s) => { if (s === "done" || s === "failed") ended = s; }).catch(() => {}); }, 2500);
    return () => clearInterval(t);
  });
  function send() { if (!code.trim()) return; sent = true; onanswer(code.trim()); }
</script>

<div class="si" role="group" aria-label={request.label} data-state={shown ?? "pending"}>
  {#if shown === "done"}
    <p class="done" role="status"><Icon name="check" inline /> Authentication done</p>
  {:else if shown === "failed"}
    <p class="bad" role="status"><Icon name="warning" inline /> Signing in to {request.providerId} did not work. Send a message to try again.</p>
  {:else}
    <p class="what"><Icon name="key" inline /> {request.label}</p>
    {#if request.url}
      <p class="step">Open <a href={request.url} target="_blank" rel="noopener noreferrer">{request.url}</a> and sign in.</p>
    {/if}
    {#if request.displayCode}<p class="step">Type this code there: <code class="code">{request.displayCode}</code></p>{/if}
    {#if request.needsCode}
      <form onsubmit={(e) => { e.preventDefault(); send(); }}>
        <label class="visually-hidden" for="si-code-{request.id}">Code from the provider</label>
        <input id="si-code-{request.id}" autocomplete="off" spellcheck="false" bind:value={code} placeholder="Paste the code it shows" disabled={sent} />
        <Button variant="primary" text={sent ? "Checking…" : "Continue"} disabled={!code.trim() || sent} type="submit" />
      </form>
    {:else if !request.url && !request.displayCode}
      <p class="step">Sign in to the provider on the machine that runs this thread, then send a message.</p>
    {/if}
  {/if}
</div>

<style>
  .si { display: grid; gap: var(--s-3); padding: var(--s-5) var(--s-6); border-radius: var(--r-select); background: color-mix(in oklab, var(--accent-base) 10%, transparent); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--accent-base) 28%, transparent); font-size: var(--text-ui); }
  p { margin: 0; color: var(--ink-2); }
  .what { color: var(--ink); font-weight: var(--strong-weight); display: inline-flex; align-items: center; gap: var(--s-3); }
  .done { color: var(--ink); display: inline-flex; align-items: center; gap: var(--s-3); }
  .bad { color: var(--danger); display: inline-flex; align-items: center; gap: var(--s-3); }
  a { color: var(--accent); text-decoration: underline; overflow-wrap: anywhere; }
  .code { font-size: var(--text-heading); letter-spacing: 0.06em; color: var(--ink); user-select: text; }
  form { display: flex; flex-wrap: wrap; gap: var(--s-3); align-items: center; }
  form input { flex: 1 1 12rem; min-height: var(--pill-h); padding: 0 var(--s-5); border: 0; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink); font-size: var(--text-ui); }
</style>
