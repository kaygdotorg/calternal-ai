<script lang="ts">
  // The leading image of a row (revision 13): a small host-authorized image read through the host's row-image route and kept in memory only, keyed by its ref, for as long as the row shows it. While it loads, or when the host
  // has none (404, expired, stale), the declared fallback draws a simple document miniature from the freshly authorised snippet, and without text when there is no snippet: never a blank. The renderer never fetches anything
  // but a host-issued ref through the session, and the app's cache bounds, shares and cancels the reads.
  import { getContext } from "svelte";
  import { IMAGES_KEY, type ImageReader } from "../invoke/context";
  let { ref, snippet }: { ref: string | undefined; snippet: string | undefined } = $props();
  const reader = getContext<ImageReader | undefined>(IMAGES_KEY);
  let url = $state<string | null>(null);
  $effect(() => {
    const r = ref; url = null;
    if (!r || !reader) return;
    const ctl = new AbortController(); // the row left the screen: the read stops (the cache keeps what it has)
    void reader(r, ctl.signal).then((u) => { if (!ctl.signal.aborted) url = u; });
    return () => ctl.abort();
  });
  const lines = $derived((snippet ?? "").replace(/\s+/g, " ").trim().slice(0, 90));
</script>

<span class="img" data-row-image>
  {#if url}<img src={url} alt="" draggable="false" />
  {:else}<span class="mini" data-miniature={lines ? "text" : "blank"} aria-hidden="true">{#if lines}<span class="tx">{lines}</span>{:else}<i></i><i></i><i></i>{/if}</span>{/if}
</span>

<style>
  .img { flex: none; display: grid; width: 30px; height: 36px; border-radius: 4px; overflow: hidden; }
  img { width: 100%; height: 100%; object-fit: cover; }
  .mini { display: block; width: 100%; height: 100%; box-sizing: border-box; padding: 4px 3px; background: color-mix(in oklab, var(--ink) 6%, var(--paper, white)); border: 1px solid color-mix(in oklab, var(--ink) 14%, transparent); border-radius: 4px; overflow: hidden; }
  .tx { display: block; font-size: 4.5px; line-height: 5.5px; color: var(--ink-2); word-break: break-all; overflow: hidden; max-height: 100%; }
  i { display: block; height: 2px; margin: 3px 0; border-radius: 1px; background: color-mix(in oklab, var(--ink) 20%, transparent); } i:nth-child(2) { width: 80%; } i:nth-child(3) { width: 55%; }
</style>
