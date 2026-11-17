<script lang="ts">
  // The sticky bar above the composer: everything an agent is waiting on in this thread, one request at a time.
  //   question  a form described in generated UI, drawn here; answers go into the person's message. Never expires.
  //   approval  drawn by the shell only (never by generated UI); an agent cannot answer it. Never expires.
  //   secret    drawn by the shell only; masked entry, the agent only ever gets a handle; expires after 15 minutes and can be asked again.
  // Answering is an operation (`onanswer`), so the same answer works from any device or the CLI. The bar is absent when nothing waits.
  import { Badge, Button, Glass, Icon, IconButton, adaptive } from "@calternal/ui";
  import GenUi from "../../genui/GenUi.svelte";
  import type { Value } from "@calternal/genui";
  import type { Answer, Request } from "./model";

  let { requests, onanswer, now = () => Date.now() }: { requests: Request[]; onanswer: (id: string, a: Answer) => void; now?: () => number } = $props();
  let index = $state(0);
  let folded = $state(false);
  let formState = $state<Record<string, Value>>({});
  let secret = $state("");
  let tick = $state(Date.now());
  const cur = $derived(requests[Math.min(index, requests.length - 1)]);
  const hasSecret = $derived(requests.some((r) => r.kind === "secret"));
  $effect(() => {
    if (!hasSecret) return;
    const t = setInterval(() => (tick = now()), 1000);
    return () => clearInterval(t);
  });
  const left = $derived(cur?.kind === "secret" ? Math.max(0, cur.expiresAt - tick) : 0);
  const clock = $derived(`${Math.floor(left / 60000)}:${String(Math.floor((left % 60000) / 1000)).padStart(2, "0")}`);
  const expired = $derived(cur?.kind === "secret" && left === 0);
  const ICON = { question: "question", approval: "shield", secret: "key" } as const;
  const NAME = { question: "Question", approval: "Approval needed", secret: "Secret needed" } as const;
  function answer(a: Answer) {
    if (!cur) return;
    onanswer(cur.id, a);
    secret = "";
    index = 0;
  }
</script>

{#if cur}
  <section class="bar" aria-label="Waiting on you" data-kind={cur.kind} use:adaptive>
    <Glass dense />
    <header>
      <span class="kind"><Icon name={ICON[cur.kind]} mini />{NAME[cur.kind]}</span>
      {#if requests.length > 1}
        <span class="pager">
          <IconButton icon="back" label="Previous request" disabled={index === 0} onclick={() => (index = Math.max(0, index - 1))} />
          <span aria-live="polite">{index + 1} of {requests.length}</span>
          <IconButton icon="chevron" label="Next request" disabled={index >= requests.length - 1} onclick={() => (index = Math.min(requests.length - 1, index + 1))} />
        </span>
      {/if}
      <IconButton icon={folded ? "up" : "down"} label={folded ? "Show the request" : "Fold the request"} pressed={folded} onclick={() => (folded = !folded)} />
    </header>

    {#if !folded}
      <div class="body">
        {#if cur.kind === "question"}
          <GenUi source={cur.form} bind:values={formState} />
          <footer><Button variant="primary" text="Send answers" onclick={() => answer({ kind: "question", values: { ...formState } })} /></footer>
        {:else if cur.kind === "approval"}
          <div class="trusted"><Icon name="lock" mini /> Drawn by Calternal. An agent can’t answer this for you.</div>
          <h3>{cur.title}</h3>
          <p>{cur.action}</p>
          <pre aria-label="The command"><code>{cur.command}</code></pre>
          <footer>
            <Button variant="quiet" text="Deny" onclick={() => answer({ kind: "approval", decision: "deny" })} />
            <Button text="Allow for this thread" onclick={() => answer({ kind: "approval", decision: "thread" })} />
            <Button variant="primary" text="Allow once" onclick={() => answer({ kind: "approval", decision: "once" })} />
          </footer>
        {:else}
          <div class="trusted"><Icon name="lock" mini /> Drawn by Calternal. The agent gets a handle, never the value.</div>
          <h3>{cur.label}</h3>
          <p id="why">{cur.why}</p>
          <div class="meta"><Badge text={cur.scope === "personal" ? "Personal" : "Shared with the space"} tone="info" /><Badge text={cur.required ? "Required" : "Optional"} /><code>{cur.name}</code></div>
          {#if expired}
            <p class="exp">This request expired after 15 minutes.</p>
            <footer><Button variant="primary" text="Ask again" onclick={() => answer({ kind: "secret-skip" })} /></footer>
          {:else}
            <form onsubmit={(e) => { e.preventDefault(); if (secret) answer({ kind: "secret", value: secret }); }}>
              <label class="visually-hidden" for="secret">{cur.label}</label>
              <input id="secret" type="password" autocomplete="off" spellcheck="false" bind:value={secret} aria-describedby="why" placeholder="Paste the value" />
              <Button variant="quiet" text={cur.required ? "Not now" : "Skip"} onclick={() => answer({ kind: "secret-skip" })} />
              <Button variant="primary" text="Save" disabled={!secret} type="submit" />
            </form>
            <p class="clock">Expires in <time>{clock}</time></p>
          {/if}
        {/if}
      </div>
    {/if}
  </section>
{/if}

<style>
  .bar { position: relative; isolation: isolate; border-radius: var(--r-panel); color: var(--ink); max-height: min(26rem, 52dvh); overflow: auto; flex: 0 1 auto; }
  .bar[data-kind="approval"], .bar[data-kind="secret"] { box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--accent-base) 38%, transparent); }
  header { display: flex; align-items: center; gap: var(--s-4); padding: var(--s-2) var(--s-2) 0 var(--s-6); }
  .kind { flex: 1; display: inline-flex; align-items: center; gap: var(--s-3); font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--accent); text-transform: uppercase; letter-spacing: 0.04em; }
  .pager { display: inline-flex; align-items: center; font-size: var(--text-label); color: var(--ink-3); font-variant-numeric: tabular-nums; }
  .body { display: grid; gap: var(--s-4); padding: 0 var(--s-6) var(--s-6); }
  h3 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  p { color: var(--ink-2); font-size: var(--text-ui); }
  .trusted { display: inline-flex; align-items: center; gap: var(--s-3); font-size: var(--text-label); color: var(--ink-2); }
  pre { margin: 0; padding: var(--s-4) var(--s-5); border-radius: var(--r-control); background: rgb(var(--ink-rgb) / 0.08); overflow-x: auto; font-size: var(--text-label); }
  footer, form { display: flex; flex-wrap: wrap; justify-content: flex-end; align-items: center; gap: var(--s-3); }
  form input { flex: 1 1 12rem; min-height: max(var(--pill-h), var(--hit-min)); padding: 0 var(--s-5); border: 0; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink); font-size: var(--text-ui); }
  .meta { display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-3); }
  .meta code { font-size: var(--text-label); color: var(--ink-3); }
  .clock { font-size: var(--text-label); color: var(--ink-3); }
  .exp { color: var(--danger); }
</style>
