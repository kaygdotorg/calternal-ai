<script lang="ts">
  // The one picker behind `@`, `/`, `[[` and `#`: a listbox grouped by kind, drawn in a Popover next to the field. The thread composer and the note editor both use it; each owns what is
  // typed, which groups to show and what choosing does. The highlighted row is `sel` (an index into the groups' entries in order); the host moves it with the keys.
  import { Icon, Popover, type Anchor, type IconName } from "@calternal/ui";
  import type { Mention, MentionGroup } from "./mentions";
  let { groups, sel = $bindable(0), anchor, prefer = "top", label, onchoose, onclose }: {
    groups: MentionGroup[]; sel?: number; anchor: Anchor | null | undefined; prefer?: "top" | "bottom"; label: string; onchoose: (m: Mention) => void; onclose: () => void;
  } = $props();
  const ICON: Record<string, IconName> = { person: "user", agent: "spark", thread: "threads", item: "notes", skill: "wrench", tag: "list", block: "plus" };
  const flat = $derived(groups.flatMap((g) => g.entries));
  // The list appears two frames after the key that asked for it: the key's own frame paints its character, the next one is free for the key typed right after, and the popover's mount
  // (its portal, its material and its placement, about 9 ms at 4x CPU slowdown) is a frame of its own, never inside a key's. It closes at once. (The field reports a trigger on the `input` event
  // itself, one event earlier than the caret tracking this replaced, so one frame of delay is no longer enough to keep the mount out of the key's frame.)
  let shown = $state(false);
  $effect(() => {
    if (!groups.length) { shown = false; return; }
    if (shown) return;
    let id = requestAnimationFrame(() => { id = requestAnimationFrame(() => (shown = true)); });
    return () => cancelAnimationFrame(id);
  });
</script>

<Popover open={shown && groups.length > 0} {anchor} {prefer} role="listbox" {label} {onclose}>
  <div class="mp">
    {#each groups as g (g.kind)}
      <div class="mg" role="group" aria-label={g.title}>
        <h3>{g.title}</h3>
        {#each g.entries as m (m.id)}
          {@const i = flat.indexOf(m)}
          <div class="mo" class:on={i === sel} role="option" tabindex="-1" aria-selected={i === sel} onpointermove={() => (sel = i)} onpointerdown={(e) => { e.preventDefault(); onchoose(m); }}><Icon inline name={ICON[m.kind]!} /><span class="l">{m.label}</span><span class="g">{m.sub}</span></div>
        {/each}
      </div>
    {/each}
  </div>
</Popover>

<style>
  .mp { display: grid; gap: var(--s-2); min-width: 16rem; max-width: min(26rem, 90vw); padding: var(--s-2); }
  .mg h3 { padding: var(--s-2) var(--s-4); font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-3); }
  .mo { display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h); padding: 0 var(--s-4); border-radius: var(--r-select); cursor: pointer; font-size: var(--text-ui); }
  .mo.on { background: var(--accent-soft); }
  .mo .l { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .mo .g { font-size: var(--text-label); color: var(--ink-3); }
</style>
