<script lang="ts">
  // The first-owner onboarding (and the shorter invited path): one centred card on the app's backdrop with a stepper above it, in the style of infraternal's setup wizard. Numbered steps that turn into ticks; a step that finishes
  // confirms (its tick draws on) and moves on by itself after a short beat; waits count up; nothing is faked. Skipping is offered only where it is safe and says what it costs. Where you got to is kept on this device, so a reload
  // resumes. Keyboard: Tab and Enter do everything; the heading of each step takes focus when it appears and a live line says which step it is. Reduced motion: no slide, no swell, only fades. Motion is the shell's tokens.
  import { Button, Card, Tick } from "@calternal/ui";
  import Backdrop from "../Backdrop.svelte";
  import { after, before, clear, load, NAMES, path, save, skippable, SKIP_NOTE, type StepId, type Who } from "./flow";
  import { fixtureApi, serverApi, type OnboardingApi } from "./api";
  import StepCli from "./StepCli.svelte";
  import StepDirect from "./StepDirect.svelte";
  import StepLook from "./StepLook.svelte";
  import StepMachine from "./StepMachine.svelte";
  import StepProvider from "./StepProvider.svelte";
  import StepRepos from "./StepRepos.svelte";
  import StepYou from "./StepYou.svelte";
  let { who: whoProp = "owner", token = "", demo = false, attached = false }: { who?: Who; token?: string; demo?: boolean; attached?: boolean } = $props();
  // svelte-ignore state_referenced_locally
  // A pretended run (fixtures) keeps its place in the tab's session storage, so it never leaves anything for the real onboarding to resume.
  // svelte-ignore state_referenced_locally
  const store: Storage = demo ? sessionStorage : localStorage;
  // svelte-ignore state_referenced_locally
  const saved = load(store);
  // svelte-ignore state_referenced_locally
  const who: Who = saved?.who ?? whoProp;
  // svelte-ignore state_referenced_locally
  let coder = $state(saved?.coder ?? false), step = $state<StepId>(saved?.step ?? "you"), done = $state<StepId[]>(saved?.done ?? []), skipped = $state<StepId[]>(saved?.skipped ?? []), confirming = $state<StepId | null>(null), keyed = $state(false);
  // svelte-ignore state_referenced_locally
  const api: OnboardingApi = demo ? fixtureApi(attached) : serverApi;
  const steps = $derived(path(coder));
  const TITLE: Record<StepId, string> = { you: "Who you are", look: "Choose a look", machine: "Your machine", provider: "Sign in to a provider", repos: "Your repositories", cli: "The command line", direct: "Direct mode", done: "You are set up" };
  let heading = $state<HTMLElement>();
  const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
  const persist = () => save({ who, coder, step, done, skipped }, store);
  $effect(() => { void step; void coder; persist(); queueMicrotask(() => heading?.focus({ preventScroll: true })); });
  // A key-driven finish shows its tick at once (a keyboard user is not made to watch); a pointer-driven one draws it on.
  const onKey = () => (keyed = true), onPointer = () => (keyed = false);
  /** The step is answered: its tick draws on, then (after a beat) the next step takes the card. */
  function finish(s: StepId) {
    if (confirming) return;
    if (!done.includes(s)) done = [...done, s];
    confirming = s;
    setTimeout(() => { confirming = null; const n = after(coder, s); if (n) step = n; }, reduced() ? 0 : 520);
  }
  function skip(s: StepId) { if (!skipped.includes(s)) skipped = [...skipped, s]; const n = after(coder, s); if (n) step = n; }
  function back() { const p = before(coder, step); if (p) step = p; }
  function start() {
    clear(store);
    location.replace(import.meta.env.BASE_URL + "threads");
  }
  const cur = $derived(steps.indexOf(step));
</script>

<svelte:window onkeydown={onKey} onpointerdown={onPointer} />
<Backdrop />
<main class="wrap">
  <p class="mark" aria-hidden="true">Calternal</p>
  <ol class="steps" class:many={steps.length > 5} aria-label="Setup steps">
    {#each steps as s, i (s)}
      <li class:now={s === step} class:ok={done.includes(s)} aria-current={s === step ? "step" : undefined}>
        <span class="dot">{#if done.includes(s)}<Tick checked animate={!keyed} size="var(--s-9)" />{:else}<span class="num">{i + 1}</span>{/if}</span>
        <span class="lab">{NAMES[s]}{skipped.includes(s) ? ", skipped" : ""}</span>
      </li>
    {/each}
  </ol>
  <Card label="Setup, step {cur + 1} of {steps.length}: {TITLE[step]}" variant="solid" class="card">
    {#key step}
      <section class="in">
        <h1 bind:this={heading} tabindex="-1">{TITLE[step]}</h1>
        {#if step === "you"}<StepYou {who} {token} {demo} bind:coder ondone={() => finish("you")} />
        {:else if step === "look"}<StepLook ondone={() => finish("look")} />
        {:else if step === "machine"}<StepMachine {api} ondone={() => finish("machine")} />
        {:else if step === "provider"}<StepProvider {api} ondone={() => finish("provider")} />
        {:else if step === "repos"}<StepRepos {api} ondone={() => finish("repos")} />
        {:else if step === "cli"}<StepCli ondone={() => finish("cli")} />
        {:else if step === "direct"}<StepDirect {api} ondone={() => finish("direct")} />
        {:else}
          <p class="ready"><Tick checked animate={!keyed} size="var(--s-10)" /><span>{who === "owner" ? "Your instance is ready. Start the first thread." : "You have joined. Start your first thread."}</span></p>
          <div class="go"><Button variant="primary" text="Start the first thread" onclick={start} /></div>
        {/if}
        {#if step !== "done"}
          <footer>
            {#if before(coder, step) && step !== "you"}<Button variant="quiet" text="Back" onclick={back} />{/if}
            {#if skippable[step]}<span class="skip"><Button variant="quiet" text="Skip" onclick={() => skip(step)} /><small>{SKIP_NOTE[step]}</small></span>{/if}
          </footer>
        {/if}
      </section>
    {/key}
  </Card>
  <p class="sr" role="status" aria-live="polite">Step {cur + 1} of {steps.length}: {TITLE[step]}{confirming ? ", done" : ""}</p>
</main>

<style>
  .wrap { position: relative; z-index: 1; min-height: 100dvh; display: grid; align-content: center; justify-items: center; gap: var(--s-6); padding: calc(var(--safe-top) + var(--s-6)) calc(var(--safe-right) + var(--s-6)) calc(var(--safe-bottom) + var(--s-6)) calc(var(--safe-left) + var(--s-6)); }
  .mark { font-family: var(--font-brand, inherit); font-size: var(--text-title); font-weight: var(--strong-weight); color: var(--ink); }
  .steps { display: flex; gap: var(--s-6); list-style: none; padding: 0; margin: 0; align-items: flex-start; }
  .steps li { display: grid; justify-items: center; gap: var(--s-2); width: 4.5rem; position: relative; color: var(--ink-3); transition: color var(--dur-base) var(--ease-out); }
  .steps li.now { color: var(--ink); }
  .steps li:not(:last-child)::after { content: ""; position: absolute; top: calc(var(--s-9) / 2); left: calc(50% + var(--s-9) / 2 + var(--s-2)); width: calc(100% + var(--s-6) - var(--s-9) - var(--s-4)); height: 1px; background: rgb(var(--ink-rgb) / 0.2); }
  .dot { width: var(--s-9); height: var(--s-9); display: grid; place-items: center; border-radius: 50%; font-size: var(--text-label); font-weight: var(--strong-weight); background: rgb(var(--ink-rgb) / 0.08); transition: background-color var(--dur-base) var(--ease-out); }
  .now .dot { background: var(--accent-soft, rgb(var(--ink-rgb) / 0.16)); }
  .ok .dot { background: transparent; }
  .lab { font-size: var(--text-label); text-align: center; }
  /* The longer path (eight steps): only the current name is shown, under its number. */
  .many { gap: var(--s-4); padding-bottom: var(--s-6); }
  .many li { width: 3rem; }
  .many .lab { display: none; }
  .many li.now .lab { display: block; position: absolute; top: calc(var(--s-9) + var(--s-2)); white-space: nowrap; }
  @media (max-width: 520px) { .steps { gap: var(--s-4); } .steps li { width: 3.2rem; } .lab { display: none; } .steps li.now .lab { display: block; position: absolute; top: calc(var(--s-9) + var(--s-2)); white-space: nowrap; } .steps { padding-bottom: var(--s-6); } }
  .wrap :global(.card) { width: min(30rem, 100%); }
  .in { display: grid; gap: var(--s-5); padding: var(--s-7); animation: enter var(--dur-slow) var(--ease-out); }
  h1 { font-size: var(--text-heading); font-weight: var(--strong-weight); outline: none; }
  footer { display: flex; align-items: center; gap: var(--s-4); flex-wrap: wrap; }
  .skip { display: flex; align-items: center; gap: var(--s-4); margin-inline-start: auto; }
  .skip small { color: var(--ink-3); font-size: var(--text-label); max-width: 16rem; }
  .ready { display: flex; align-items: center; gap: var(--s-5); }
  .go { display: flex; gap: var(--s-4); }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
  @keyframes enter { from { opacity: 0; translate: 0 var(--s-4); } }
  @media (prefers-reduced-motion: reduce) { .in { animation: fade var(--dur-fast) linear; } @keyframes fade { from { opacity: 0; } } }
</style>
