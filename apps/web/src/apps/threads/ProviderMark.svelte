<script lang="ts">
  // Which provider wrote an assistant message, as its icon at the message's left; the model, reasoning level and how long the turn took are in its tooltip (hover, long press) and its accessible name, so
  // scrolling back through a thread tells what ran each turn. Quiet: it is a mark, not a control (its details are its accessible name, and the tooltip on hover or long press).
  import { Icon, tip } from "@calternal/ui";
  import type { MessageBy, ProviderId } from "./model";
  let { by, provider, providers }: { by?: MessageBy; provider: ProviderId; providers: { id: ProviderId; name: string; models: { id: string; name: string }[] }[] } = $props();
  const id = $derived(by?.provider ?? provider);
  const p = $derived(providers.find((x) => x.id === id));
  const model = $derived(by?.model ? p?.models.find((m) => m.id === by.model)?.name ?? by.model : "");
  const took = $derived(by?.ms ? (by.ms >= 60_000 ? `${Math.floor(by.ms / 60_000)} min ${Math.round((by.ms % 60_000) / 1000)} s` : `${Math.max(1, Math.round(by.ms / 1000))} s`) : "");
  const label = $derived([p?.name ?? "Agent", model && model !== "default" ? model : "", by?.reasoning ? `${by.reasoning} reasoning` : "", took ? `took ${took}` : ""].filter(Boolean).join(" · "));
</script>

<span class="mark" role="img" aria-label={label} use:tip={{ label, side: "right" }}><Icon name={id === "codex" ? "codex" : "spark"} inline /></span>

<style>
  .mark { display: inline-grid; place-items: center; width: var(--s-8); height: var(--s-8); color: var(--ink-3); border-radius: 50%; }
</style>
