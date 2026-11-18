<script lang="ts">
  // Which provider wrote an assistant message, as its own icon at the message's left (Claude's mark, Codex's; the generic spark only for a plugin this client does not know). Hovering it (or focusing it from the keyboard) opens a
  // card with the exact harness, provider, model, reasoning, service tier and context (ModelCard.svelte) on the shared floating material, with the warm tooltip's timing: the first one waits 400 ms, another within 600 ms of the last
  // one's closing shows at once. It is a button so the keyboard reaches it; Escape, leaving, scrolling or blurring close it. Pointer and keyboard only: on touch a press opens it and the next press anywhere closes it.
  import { tick } from "svelte";
  import { Icon, Panel, place } from "@calternal/ui";
  import ModelCard from "./ModelCard.svelte";
  import { serverMode } from "../../server/client";
  import type { MessageBy, ProviderId } from "./model";
  let { by, provider, providers, row = false }: { /** The mark sits beside a block row (a thought, tool steps, the working row) rather than a line of text: it is centred on that row, not on the first text line. */ row?: boolean; by?: MessageBy; provider: ProviderId; providers: { id: ProviderId; name: string; models: { id: string; name: string; levels?: string[] }[] }[] } = $props();
  const id = $derived(by?.provider ?? provider);
  const p = $derived(providers.find((x) => x.id === id));
  const model = $derived(by?.model ? p?.models.find((m) => m.id === by.model)?.name ?? by.model : "");
  const took = $derived(by?.ms ? (by.ms >= 60_000 ? `${Math.floor(by.ms / 60_000)} min ${Math.round((by.ms % 60_000) / 1000)} s` : `${Math.max(1, Math.round(by.ms / 1000))} s`) : "");
  const label = $derived([p?.name ?? "Agent", by?.service ? `via ${by.service}` : "", model && model !== "default" ? model : "", by?.reasoning ? `${by.reasoning} reasoning` : "", took ? `took ${took}` : ""].filter(Boolean).join(" · "));
  const icon = $derived(id === "codex" ? "codex" : id === "claude" ? "claude" : "spark");

  let open = $state(false);
  let btn = $state<HTMLButtonElement>(), box = $state<HTMLElement>();
  // The fixtures offer both designs; the real build shows the detailed one.
  let compact = $state(!serverMode() && localStorage.getItem("cal.modelcard") === "compact");
  const toggle = () => { compact = !compact; try { localStorage.setItem("cal.modelcard", compact ? "compact" : "detailed"); } catch { /* storage is off */ } stuck = true; void tick().then(fit); addEventListener("pointermove", away, true); };
  const away = (e: PointerEvent) => { const t = e.target as Node; if (box?.contains(t) || btn?.contains(t)) return; removeEventListener("pointermove", away, true); stuck = false; hide(120); };
  // After the switch the card changes size under a still pointer; it stays open until the pointer has been in it again and left (or Escape, scrolling or blurring close it).
  let stuck = false;
  const fit = () => { if (btn && box) place(box, btn, { prefer: "bottom", gap: 8 }); };
  const toBody = (n: HTMLElement) => { document.body.append(n); return { destroy: () => n.remove() }; };

  let timer = 0;
  const WARM = 600, DELAY = 400;
  const win = window as unknown as { __cardClosed?: number };
  function show(now = false) {
    clearTimeout(timer);
    const warm = performance.now() - (win.__cardClosed ?? -1e9) < WARM;
    timer = window.setTimeout(() => { open = true; void tick().then(fit); }, now || warm ? 0 : DELAY);
  }
  function hide(grace = 0) {
    clearTimeout(timer);
    if (stuck && grace > 0) return;
    timer = window.setTimeout(() => { if (open) win.__cardClosed = performance.now(); open = false; }, grace);
  }
  $effect(() => {
    if (!open) return;
    const close = () => hide();
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") { stuck = false; hide(); } };
    addEventListener("scroll", close, { capture: true, passive: true }); addEventListener("keydown", key);
    return () => { removeEventListener("scroll", close, { capture: true } as EventListenerOptions); removeEventListener("keydown", key); };
  });
</script>

<button type="button" class="mark" class:row aria-label={label} aria-describedby={open ? `mc-${by?.model ?? id}` : undefined} bind:this={btn}
  onpointerenter={(e) => { stuck = false; if (e.pointerType !== "touch") show(); }} onpointerleave={() => hide(120)}
  onfocus={(e) => { if ((e.currentTarget as HTMLElement).matches(":focus-visible")) show(true); }} onblur={() => hide()}
  onclick={(e) => { const ev = e as unknown as PointerEvent; if (ev.pointerType === "touch" || ev.detail === 0) { open ? hide() : show(true); } }}><Icon name={icon} inline /></button>

{#if open}
  <div class="host" use:toBody onpointerenter={() => { stuck = false; clearTimeout(timer); }} onpointerleave={() => hide(120)} role="tooltip" id="mc-{by?.model ?? id}">
    <Panel floating dense radius="card" bind:el={box} class="mc"><ModelCard {by} {provider} {providers} {compact} ontoggle={serverMode() ? undefined : toggle} /></Panel>
  </div>
{/if}

<style>
  /* An inline mark, aligned to the first line's x-height centre by the line box (`vertical-align: middle` puts its centre at the baseline plus half the parent's x-height), never by a pixel nudge; the margin makes the box one line tall, so it never pushes the line's baseline off the text beside it. */
  .mark.row { margin-block: 0; vertical-align: top; }
  .mark { display: inline-grid; place-items: center; vertical-align: middle; width: var(--s-8); height: var(--s-8); margin-block: calc((1lh - var(--s-8)) / 2); color: var(--ink-2); border-radius: 50%; }
  @media (hover: hover) and (pointer: fine) { .mark:hover { background: var(--hover); color: var(--ink); } }
  .host { position: fixed; inset: 0 auto auto 0; z-index: 60; pointer-events: none; }
  .host :global(.mc) { position: fixed; left: 0; top: 0; pointer-events: auto; z-index: 60; }
</style>
