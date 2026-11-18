<script lang="ts">
  // The sticky bar above the composer: everything an agent is waiting on in this thread, one request at a time.
  //   question  a form described in generated UI, drawn here; answers go into the person's message. Never expires.
  //   approval  drawn by the shell only (never by generated UI); an agent cannot answer it. Never expires.
  //   secret    drawn by the shell only; masked entry, the agent only ever gets a handle; expires after 15 minutes and can be asked again.
  // Answering is an operation (`onanswer`), so the same answer works from any device or the CLI. The bar is absent when nothing waits.
  import { untrack, type Component } from "svelte";
  import { Badge, Button, Icon, IconButton, Panel } from "@calternal/ui";
  import type { Value } from "@calternal/genui";
  import type { Answer, Request } from "./model";
  import SignIn from "./SignIn.svelte";

  let { requests, onanswer, now = () => Date.now() }: { requests: Request[]; onanswer: (id: string, a: Answer) => void; now?: () => number } = $props();
  // The generated-UI renderer is its own chunk, fetched when the first question form is about to be drawn.
  let GenUi = $state<Component<{ source: string; values: Record<string, Value> }>>();
  $effect(() => { if (requests.some((r) => r.kind === "question") && !GenUi) void import("../../genui/GenUi.svelte").then((m) => (GenUi = m.default as never)); });
  // Touch has no hover and little room: an approval folds its details there.
  const touch = typeof matchMedia === "function" && matchMedia("(hover: none)").matches;
  let index = $state(0);
  let folded = $state(false);
  let formState = $state<Record<string, Value>>({});
  let secret = $state("");
  let tick = $state(untrack(() => now()));
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
  const ICON = { question: "question", approval: "shield", secret: "key", signin: "key" } as const;
  const NAME = { question: "Question", approval: "Approval needed", secret: "Secret needed", signin: "Sign in needed" } as const;
  function answer(a: Answer) {
    if (!cur) return;
    onanswer(cur.id, a);
    secret = "";
    index = 0;
  }
</script>

{#if cur}
  <Panel class="bar" radius="card" dense role="region" aria-label="Waiting on you" data-kind={cur.kind}>
    <div class="head">
      <span class="kind"><Icon name={ICON[cur.kind]} inline />{NAME[cur.kind]}</span>
      {#if requests.length > 1}
        <span class="pager">
          <IconButton icon="back" label="Previous request" disabled={index === 0} onclick={() => (index = Math.max(0, index - 1))} />
          <span aria-live="polite">{index + 1} of {requests.length}</span>
          <IconButton icon="chevron" label="Next request" disabled={index >= requests.length - 1} onclick={() => (index = Math.min(requests.length - 1, index + 1))} />
        </span>
      {/if}
      <IconButton icon={folded ? "up" : "down"} label={folded ? "Show the request" : "Fold the request"} pressed={folded} onclick={() => (folded = !folded)} />
    </div>

    {#if !folded}
      <div class="body">
        {#if cur.kind === "question"}
          <p>The agent is waiting for your answer. It continues as soon as you send it.</p>
          {#if GenUi}<GenUi source={cur.form} bind:values={formState} />{/if}
          <div class="foot"><Button variant="primary" text="Send answers" onclick={() => answer({ kind: "question", values: { ...formState } })} /></div>
        {:else if cur.kind === "approval"}
          <div class="trusted"><Icon name="lock" inline /> Drawn by Calternal. An agent can’t answer this for you.</div>
          <h2 class="what">{cur.action}</h2>
          <!-- The action and its target lead in one line; the exact command and what the choices mean are one tap away (open at once on a desktop, folded on a phone). -->
          <details class="more" open={!touch}>
            <summary>Details</summary>
            <pre aria-label="The command"><code>{cur.command}</code></pre>
            <p>Nothing runs until you decide.{cur.lifetimes.includes("thread") ? " “For this thread” keeps allowing the same action here until the thread is deleted." : ""}</p>
            {#if !cur.lifetimes.includes("thread")}<p class="once">This can only be allowed once: it cannot be undone or it widens what the agent can reach, so you are asked each time.</p>{/if}
          </details>
          <div class="foot">
            <Button variant="quiet" text="Deny" onclick={() => answer({ kind: "approval", decision: "deny" })} />
            {#if cur.lifetimes.includes("thread")}<Button text="Allow for this thread" onclick={() => answer({ kind: "approval", decision: "thread" })} />{/if}
            <Button variant="primary" text="Allow once" onclick={() => answer({ kind: "approval", decision: "once" })} />
          </div>
        {:else if cur.kind === "signin"}
          {#key cur.id}<SignIn request={cur} onanswer={(code) => answer({ kind: "signin", code })} />{/key}
        {:else}
          <div class="trusted"><Icon name="lock" inline /> Drawn by Calternal. The agent gets a handle, never the value.</div>
          <h2>{cur.label}</h2>
          <p id="why">{cur.why} Paste the value below; it goes straight to Calternal’s secret store.</p>
          <div class="meta"><Badge text={cur.scope === "personal" ? "Personal" : "Shared with the space"} tone="info" /><Badge text={cur.required ? "Required" : "Optional"} /><code>{cur.name}</code></div>
          {#if expired}
            <p class="exp">This request expired after 15 minutes.</p>
            <div class="foot"><Button variant="primary" text="Ask again" onclick={() => answer({ kind: "secret-skip" })} /></div>
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
  </Panel>
{/if}

<style>
  :global(.bar) { color: var(--ink); max-height: min(26rem, 52dvh, 60cqh); overflow: auto; flex: 0 1 auto; }
  :global(.bar[data-kind="approval"]), :global(.bar[data-kind="secret"]), :global(.bar[data-kind="signin"]) { box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--accent-base) 38%, transparent); }
  .head { display: flex; align-items: center; gap: var(--s-4); padding: var(--s-2) var(--s-2) 0 var(--s-6); }
  .kind { flex: 1; display: inline-flex; align-items: center; gap: var(--s-3); font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-2); }
  .pager { display: inline-flex; align-items: center; font-size: var(--text-label); color: var(--ink-3); font-variant-numeric: tabular-nums; }
  .body { display: grid; gap: var(--s-4); padding: 0 var(--s-6) var(--s-6); }
  h2 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  p { color: var(--ink-2); font-size: var(--text-ui); }
  .trusted { display: inline-flex; align-items: center; gap: var(--s-3); font-size: var(--text-label); color: var(--ink-2); }
  pre { margin: 0; padding: var(--s-4) var(--s-5); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.08); overflow-x: auto; font-size: var(--text-label); }
  .foot, form { display: flex; flex-wrap: wrap; justify-content: flex-end; align-items: center; gap: var(--s-3); }
  form input { flex: 1 1 12rem; min-height: var(--pill-h); padding: 0 var(--s-5); border: 0; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink); font-size: var(--text-ui); }
  .meta { display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-3); }
  .meta code { font-size: var(--text-label); color: var(--ink-3); }
  .what { font-size: var(--text-heading); font-weight: var(--strong-weight); line-height: 1.3; overflow-wrap: anywhere; }
  .more summary { cursor: pointer; color: var(--ink-3); font-size: var(--text-label); min-height: var(--pill-h); display: flex; align-items: center; }
  .more[open] summary { margin-bottom: var(--s-2); }
  .once { font-size: var(--text-label); color: var(--ink-3); }
  .clock { font-size: var(--text-label); color: var(--ink-3); }
  .exp { color: var(--danger); }
</style>
