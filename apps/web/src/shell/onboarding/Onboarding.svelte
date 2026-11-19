<script lang="ts">
  // The first-owner onboarding (and the shorter invited path): one centred card on the app's backdrop with a stepper above it, in the style of infraternal's setup wizard. Numbered steps that turn into ticks; a step that finishes
  // confirms (its tick draws on) and moves on by itself after a short beat; waits count up; nothing is faked. Skipping is offered only where it is safe and says what it costs. Where you got to is kept on this device, so a reload
  // resumes. Keyboard: Tab and Enter do everything; the heading of each step takes focus when it appears and a live line says which step it is. Reduced motion: no slide, no swell, only fades. Motion is the shell's tokens.
  import { Button, Callout, Card, haptic, IconButton, Tick, tokenMs } from "@calternal/ui";
  import Backdrop from "../Backdrop.svelte";
  import { after, before, clear, load, NAMES, path, save, skippable, SKIP_NOTE, type StepId, type Who } from "./flow";
  import { fixtureApi, serverApi, type OnboardingApi } from "./api";
  import StepCli from "./StepCli.svelte";
  import StepDirect from "./StepDirect.svelte";
  import StepLook from "./StepLook.svelte";
  import StepPasskey2 from "./StepPasskey2.svelte";
  import StepRecovery from "./StepRecovery.svelte";
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
  // Fixtures only: `?step=machine` opens that step with the ones before it done (to look at it and test it).
  // svelte-ignore state_referenced_locally
  const jump = demo ? (new URLSearchParams(location.search).get("step") as StepId | null) : null;
  // svelte-ignore state_referenced_locally
  const first: StepId = jump && path(false).includes(jump) ? jump : saved?.step ?? "you";
  // svelte-ignore state_referenced_locally
  let coder = $state(saved?.coder ?? false), step = $state<StepId>(first), done = $state<StepId[]>(jump ? path(false).slice(0, path(false).indexOf(first)) : saved?.done ?? []), skipped = $state<StepId[]>(saved?.skipped ?? []), confirming = $state<StepId | null>(null), keyed = $state(false);
  // svelte-ignore state_referenced_locally
  const api: OnboardingApi = demo ? fixtureApi(attached) : serverApi;
  const steps = $derived(path(coder));
  const TITLE: Record<StepId, string> = { you: "Who you are", recovery: "Your recovery codes", passkey2: "A second passkey", look: "Choose a look", machine: "Your machine", provider: "Sign in to a provider", repos: "Your repositories", cli: "The command line", direct: "Direct mode", done: "You are set up" };
  let heading = $state<HTMLElement>();
  /** A step reports that it needs nothing more (a machine is connected, a provider is signed in): Skip and its warning are then hidden. */
  let satisfied = $state(false);
  // On the last screen: if nothing is signed in to answer, say so before the first thread (the composer there offers a provider whether or not it is signed in).
  let notice = $state("");
  $effect(() => {
    if (shownStep !== "done") { notice = ""; return; }
    let live = true;
    void api.providers().then((rows) => { if (!live) return; notice = rows.some((r) => r.state === "signed_in") ? "" : rows.length ? "No provider is signed in on your machine yet, so a thread will not get an answer. Sign in under Settings, Machines first." : "No machine has a provider to sign in to yet, so a thread will not get an answer. Add a machine, turn on Claude Code or Codex, and sign in under Settings, Machines."; }, () => {});
    return () => { live = false; };
  });
  // The card's content moves between steps in two beats: the old content leaves (`out`, `--dur-fast`), the new one is placed at the side the move came from (`pre`, no transition) and then eases in (`in`, `--dur-base`).
  // All of it is CSS transitions, so a move made while another is running turns round from where it is; `dir` is 1 for forward and -1 for back.
  let dir = $state<1 | -1>(1), shownStep = $state<StepId>(first), phase = $state<"out" | "pre" | "in">("pre"), swap = 0;
  const settle = () => requestAnimationFrame(() => requestAnimationFrame(() => (phase = "in")));
  $effect(() => { if (shownStep === step) return; phase = "out"; clearTimeout(swap); swap = window.setTimeout(() => { shownStep = step; phase = "pre"; settle(); }, reduced() ? 0 : tokenMs("--dur-fast", 150)); return () => clearTimeout(swap); });
  $effect(() => { settle(); });
  /** What the card's chevron does inside the current step (null: the step has no screens of its own to go back through). */
  let subBack = $state<(() => void) | null>(null);
  const canBack = $derived(subBack !== null || (!!before(coder, step) && step !== "you" && step !== "recovery" && step !== "passkey2" && step !== "done"));
  const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
  // The account this record belongs to, once there is one (after who-you-are): a reload resumes only a record of the signed-in account (server/boot.ts).
  // svelte-ignore state_referenced_locally
  let account = $state<string | undefined>(saved?.account);
  const persist = () => save({ who, coder, step, done, skipped, ...(account ? { account } : {}) }, store);
  $effect(() => { if (demo || account || step === "you") return; void import("../../server/auth").then((a) => a.account.me()).then((m) => { account = m.user.id; persist(); }, () => {}); });
  $effect(() => { void step; void coder; persist(); });
  $effect(() => { void shownStep; queueMicrotask(() => heading?.focus({ preventScroll: true })); });
  // A key-driven finish shows its tick at once (a keyboard user is not made to watch); a pointer-driven one draws it on.
  const onKey = () => (keyed = true), onPointer = () => (keyed = false);
  /** The step is answered: its tick draws on, then (after a beat) the next step takes the card. */
  function finish(s: StepId) {
    if (confirming) return;
    if (!done.includes(s)) done = [...done, s];
    confirming = s;
    haptic("success"); // a step is done (a native tick inside the iPhone shell; nothing in a browser)
    setTimeout(() => { confirming = null; const n = after(coder, s); if (n) { dir = 1; step = n; } }, reduced() ? 0 : 520);
  }
  function skip(s: StepId) { if (s === "passkey2" && !demo) void import("../../server/auth").then((a) => a.dismissPasskeyPrompt()).catch(() => {}); if (!skipped.includes(s)) skipped = [...skipped, s]; const n = after(coder, s); if (n) { dir = 1; step = n; } }
  function back() { const p = before(coder, step); if (p) { dir = -1; step = p; } }
  const goBack = () => { if (subBack) subBack(); else back(); };
  async function start() {
    if (!demo) await import("../../server/client").then(({ server }) => server.operate("onboarding.finish", {})).catch(() => {}); // the server remembers it is finished, for every browser
    clear(store);
    location.replace(import.meta.env.BASE_URL + "threads");
  }
  const cur = $derived(steps.indexOf(step));
  // With the keyboard up on a phone, the iOS form bar (previous, next, done) sits over the bottom of what the page can see: after a field takes focus the card's action button is scrolled to just above it, so Continue is never under the keyboard.
  $effect(() => {
    const onFocus = (e: FocusEvent) => { if (!(e.target instanceof HTMLElement) || !/^(INPUT|TEXTAREA)$/.test(e.target.tagName)) return; setTimeout(() => document.querySelector<HTMLElement>(".wrap button[type=submit]")?.scrollIntoView({ block: "nearest", behavior: reduced() ? "auto" : "smooth" }), 400); };
    addEventListener("focusin", onFocus);
    return () => removeEventListener("focusin", onFocus);
  });
</script>

<svelte:window onkeydown={onKey} onpointerdown={onPointer} />
<Backdrop />
<main class="wrap">
  <p class="mark" aria-hidden="true">Calternal</p>
  <ol class="steps" class:many={steps.length > 5} aria-label="Setup steps">
    {#each steps as s, i (s)}
      <li style:--i={i} class:now={s === step} class:ok={done.includes(s)} aria-current={s === step ? "step" : undefined}>
        <span class="dot" class:skipped={skipped.includes(s)}>{#if done.includes(s)}<Tick checked animate={!keyed} size="var(--s-9)" />{:else if skipped.includes(s)}<span class="num" aria-hidden="true">–</span>{:else}<span class="num">{i + 1}</span>{/if}</span>
        <span class="lab">{NAMES[s]}{skipped.includes(s) ? ", skipped" : ""}</span>
      </li>
    {/each}
  </ol>
  <Card label="Setup, step {cur + 1} of {steps.length}: {TITLE[step]}" variant="solid" class="card">
    <!-- The back chevron: top left of the card, our icon button (name "Back", 44 pt), absent on the first step. The row opens and closes (it grows from nothing) so the card does not jump. -->
    <div class="bar" data-open={canBack}><div class="barclip"><div class="barrow">{#if canBack}<IconButton icon="back" label="Back" class="bk" onclick={goBack} />{/if}</div></div></div>
    {#key shownStep}
      <section class="in" data-phase={phase} style:--dir={dir}>
        <h1 bind:this={heading} tabindex="-1">{TITLE[shownStep]}</h1>
        {#if shownStep === "you"}<StepYou {who} {token} {demo} bind:coder ondone={() => finish("you")} onback={(f) => (subBack = f)} />
        {:else if shownStep === "recovery"}<StepRecovery {demo} ondone={() => finish("recovery")} />
        {:else if shownStep === "passkey2"}<StepPasskey2 {demo} ondone={() => finish("passkey2")} />
        {:else if shownStep === "look"}<StepLook ondone={() => finish("look")} />
        {:else if shownStep === "machine"}<StepMachine {api} bind:satisfied ondone={() => finish("machine")} />
        {:else if shownStep === "provider"}<StepProvider {api} bind:satisfied ondone={() => finish("provider")} />
        {:else if shownStep === "repos"}<StepRepos {api} ondone={() => finish("repos")} />
        {:else if shownStep === "cli"}<StepCli ondone={() => finish("cli")} />
        {:else if shownStep === "direct"}<StepDirect {api} ondone={() => finish("direct")} />
        {:else}
          <p class="ready"><Tick checked animate={!keyed} size="var(--s-10)" /><span>{who === "owner" ? "Your instance is ready. Start the first thread." : "You have joined. Start your first thread."}</span></p>
          <Callout message={notice} tone="warn" />
          <div class="go"><Button class="settings-action" variant="primary" text="Start the first thread" onclick={start} /></div>
        {/if}
        {#if shownStep !== "done"}
          <footer>
            {#if skippable[shownStep] && !satisfied}<span class="skip"><Button variant="quiet" text="Skip" onclick={() => skip(shownStep)} /><small>{SKIP_NOTE[shownStep]}</small></span>{/if}
          </footer>
        {/if}
      </section>
    {/key}
  </Card>
  <p class="sr" role="status" aria-live="polite">Step {cur + 1} of {steps.length}: {TITLE[step]}{confirming ? ", done" : ""}</p>
</main>

<style>
  .wrap { position: relative; z-index: 1; min-height: 100dvh; display: grid; align-content: center; justify-items: center; gap: var(--s-6); padding: calc(var(--safe-top) + var(--s-6)) calc(var(--safe-right) + var(--s-6)) calc(var(--safe-bottom) + var(--s-6)) calc(var(--safe-left) + var(--s-6)); }
  /* The script face's last "l" swings past its advance width; the box is padded so the whole stroke is inside it. Without the padding Safari paints the overhang as a separate piece while the word moves (a stray comma-like mark after the wordmark when the page re-centres). */
  .mark { font-family: var(--font-brand, inherit); font-size: var(--text-title); font-weight: var(--strong-weight); color: var(--ink); padding-inline: 0.3em; }
  .steps { display: flex; gap: var(--s-6); list-style: none; padding: 0; margin: 0; align-items: flex-start; }
  .steps li { display: grid; justify-items: center; gap: var(--s-2); width: 4.5rem; position: relative; color: var(--ink-3); transition: color var(--dur-base) var(--ease-out); }
  .steps li.now { color: var(--ink); }
  /* The line to the next step fills with the accent (from its left end, --dur-slow) once the step is done. */
  .steps li:not(:last-child)::before { content: ""; position: absolute; z-index: 1; top: calc(var(--s-9) / 2); left: calc(50% + var(--s-9) / 2 + var(--s-2)); width: calc(100% + var(--s-6) - var(--s-9) - var(--s-4)); height: 1px; background: var(--accent-base); transform-origin: 0 50%; scale: 0 1; transition: scale var(--dur-slow) var(--ease-out); }
  .steps li.ok:not(:last-child)::before { scale: 1 1; }
  .steps li:not(:last-child)::after { content: ""; position: absolute; top: calc(var(--s-9) / 2); left: calc(50% + var(--s-9) / 2 + var(--s-2)); width: calc(100% + var(--s-6) - var(--s-9) - var(--s-4)); height: 1px; background: rgb(var(--ink-rgb) / 0.2); }
  .dot { width: var(--s-9); height: var(--s-9); display: grid; place-items: center; border-radius: 50%; font-size: var(--text-label); font-weight: var(--strong-weight); background: rgb(var(--ink-rgb) / 0.08); transition: scale var(--dur-slow) var(--spring), background-color var(--dur-base) var(--ease-out); }
  .now .dot { background: var(--accent-soft, rgb(var(--ink-rgb) / 0.16)); scale: 1.08; }
  .ok .dot { background: transparent; }
  /* A skipped step shows a dash, not its number: it was passed over on purpose, and its label says so. */
  .dot.skipped { background: transparent; box-shadow: inset 0 0 0 1px rgb(var(--ink-rgb) / 0.2); color: var(--ink-3); }
  .lab { font-size: var(--text-label); text-align: center; }
  /* The longer path (eight steps): only the current name is shown, under its number. */
  .many { gap: var(--s-4); padding-bottom: var(--s-6); }
  .many li { width: 3rem; }
  .many .lab { display: none; }
  .many li.now .lab { display: block; position: absolute; top: calc(var(--s-9) + var(--s-2)); white-space: nowrap; }
  @media (max-width: 520px) { .steps { flex-wrap: wrap; justify-content: center; gap: var(--s-3) var(--s-4); } .steps li { width: 3.2rem; } .many li { width: 2.4rem; } .lab { display: none; } .steps li.now .lab { display: block; position: absolute; top: calc(var(--s-9) + var(--s-2)); white-space: nowrap; } .steps { padding-bottom: var(--s-6); } }
  .wrap :global(.card) { width: min(30rem, 100%); }
  /* The action button keeps room under it for the iOS form bar when it is scrolled into view over the keyboard; the page can scroll that far while the keyboard is up. */
  .wrap :global(button[type="submit"]) { scroll-margin-bottom: 6.5rem; }
  :global(html[data-kb]) .wrap { padding-bottom: calc(var(--s-6) + 5rem); }
  /* The content of a step: leaves toward the side it came from (`out`, --dur-fast), is placed at the side it arrives from (`pre`, no transition) and eases in (`in`, --dur-base). Transitions, so a second move turns round where it is. */
  .in { display: grid; gap: var(--s-5); padding: var(--s-7); opacity: 1; translate: 0 0; transition: opacity var(--dur-base) var(--ease-out), translate var(--dur-base) var(--ease-out), padding-top var(--dur-base) var(--ease-out); }
  .in[data-phase="out"] { opacity: 0; translate: calc(var(--dir) * var(--s-5) * -1) 0; transition-duration: var(--dur-fast); pointer-events: none; }
  .in[data-phase="pre"] { opacity: 0; translate: calc(var(--dir) * var(--s-6)) 0; transition: none; }
  /* The back chevron's row: grows open (--dur-base) so the card does not jump; the chevron itself fades in as it opens. */
  .bar { display: grid; grid-template-rows: 0fr; transition: grid-template-rows var(--dur-base) var(--ease-out); }
  .bar[data-open="true"] { grid-template-rows: 1fr; }
  .barclip { min-height: 0; overflow: hidden; }
  .barrow { padding: var(--s-3) var(--s-4) 0; }
  .bar :global(.bk) { animation: bk-in var(--dur-base) var(--ease-out) backwards; }
  .bar[data-open="true"] ~ .in { padding-top: var(--s-3); }
  @keyframes bk-in { from { opacity: 0; translate: calc(var(--s-4) * -1) 0; } }
  /* Arriving: the wordmark first, then the steps one by one, then the card (order and delays below; none under reduced motion). */
  .mark { animation: arrive var(--dur-slow) var(--ease-out) backwards; }
  .steps li { animation: arrive var(--dur-base) var(--ease-out) backwards; animation-delay: calc(90ms + var(--i, 0) * 45ms); }
  .wrap :global(.card) { animation: arrive var(--dur-slow) var(--ease-out) 220ms backwards; }
  @keyframes arrive { from { opacity: 0; translate: 0 var(--s-4); } }
  h1 { font-size: var(--text-heading); font-weight: var(--strong-weight); outline: none; }
  footer { display: flex; align-items: center; gap: var(--s-4); flex-wrap: wrap; }
  /* Skip is one quiet action with what it costs written under it, left aligned (not a button and a ragged sentence side by side). */
  .skip { display: grid; justify-items: start; gap: var(--s-1); }
  .skip small { color: var(--ink-3); font-size: var(--text-label); }
  .ready { display: flex; align-items: center; gap: var(--s-5); }
  .go { display: flex; gap: var(--s-4); }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
  /* Reduced motion keeps a short fade and drops the slide (docs/native-feel.md, 19: movement becomes opacity, at most 120 ms); the arrival delays go. */
  @media (prefers-reduced-motion: reduce) {
    .mark, .steps li, .wrap :global(.card) { animation-delay: 0ms; }
    .in, .in[data-phase="out"], .in[data-phase="pre"] { translate: 0 0 !important; transition-duration: 120ms !important; }
    .bar :global(.bk) { animation-name: none; }
  }
</style>
