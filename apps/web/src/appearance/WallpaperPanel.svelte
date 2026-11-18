<script lang="ts">
  // Wallpaper, as a section of Settings, Appearance (the same adapters as before: library.ts, so the real server plugs in unchanged): the pictures you stored, the built-in and uploaded wallpapers, a picture link, and Unsplash search.
  import { Button, Icon, textField, tip } from "@calternal/ui";
  import type { StockResult } from "./library";
  import { appearance as A } from "./prefs.svelte";

  const p = $derived(A.prefs);
  $effect(() => { void A.ensureThumbs(); });
  let query = $state("");
  let results = $state<StockResult[]>([]);
  let busy = $state("");
  let error = $state("");
  async function upload(e: Event) {
    const f = (e.currentTarget as HTMLInputElement).files?.[0];
    if (!f) return;
    error = "";
    try {
      const w = await A.library.upload(f);
      await A.refresh();
      A.change((x) => (x.wallpaperId = w.id));
    } catch (err) {
      error = (err as Error).message;
    }
  }
  let link = $state("");
  async function fromLink() {
    if (!A.library.fromLink) { error = "This library cannot take links."; return; }
    error = ""; busy = "link";
    try {
      const w = await A.library.fromLink(link);
      await A.refresh();
      A.change((x) => (x.wallpaperId = w.id));
      link = "";
    } catch (err) { error = (err as Error).message; }
    busy = "";
  }
  // Pasting a picture link anywhere in the open sheet uses it (outside a text field, which keeps its own paste).
  $effect(() => {
        const onpaste = (e: ClipboardEvent) => {
      const t = e.target as HTMLElement;
      if (t.closest("input, textarea")) return;
      const text = e.clipboardData?.getData("text")?.trim() ?? "";
      if (/^https?:\/\/\S+$/.test(text)) { link = text; void fromLink(); }
    };
    addEventListener("paste", onpaste);
    return () => removeEventListener("paste", onpaste);
  });
  const bytes = (n: number) => (n >= 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);
  const mine = $derived(A.wallpapers.filter((w) => w.bytes));
  async function removeId(id: string) {
    await A.library.remove(id);
    await A.refresh();
    if (A.prefs.wallpaperId === id) A.change((x) => (x.wallpaperId = null));
  }
  async function search() {
    error = "";
    busy = "search";
    results = await A.library.search(query);
    busy = "";
  }
  async function use(r: StockResult) {
    busy = r.id;
    const w = await A.library.save(r);
    await A.refresh();
    A.change((x) => (x.wallpaperId = w.id));
    busy = "";
  }
  async function remove() {
    const id = p.wallpaperId;
    if (!id) return;
    await A.library.remove(id);
    await A.refresh();
    A.change((x) => (x.wallpaperId = null));
  }
  const removable = $derived(A.wallpaper && A.wallpaper.source !== "builtin");

</script>

<div class="wp">
{#if mine.length}
  <section aria-labelledby="s-store">
    <h2 id="s-store">Your pictures</h2>
    <ul class="store">
      {#each mine as w (w.id)}
        <li><span class="sn">{w.name}</span><span class="note">{bytes(w.bytes!)}</span><Button variant="quiet" text="Remove" onclick={() => removeId(w.id)} /></li>
      {/each}
      <li class="tot"><span class="sn">Total</span><span class="note">{bytes(mine.reduce((s, w) => s + (w.bytes ?? 0), 0))}</span></li>
    </ul>
  </section>
{/if}

<section aria-labelledby="s-wall">
  
  <div class="grid" role="radiogroup" aria-label="Wallpaper">
    <button type="button" role="radio" class="tile none" aria-checked={p.wallpaperId === "auto"} aria-label="Automatic: follows the colour scheme" onclick={() => A.change((x) => (x.wallpaperId = "auto"))}><Icon name="sun" /></button>
    <button type="button" role="radio" class="tile none" aria-checked={p.wallpaperId === null} aria-label="No wallpaper" onclick={() => A.change((x) => (x.wallpaperId = null))}><Icon name="image" /></button>
    {#each A.wallpapers as w (w.id)}
      <button type="button" role="radio" class="tile" aria-checked={p.wallpaperId === w.id} aria-label={w.name} use:tip={{ label: w.credit ? `Photo by ${w.credit.author} on Unsplash` : "" }} style:background-image="url({A.thumbOf(w)})" onclick={() => A.change((x) => (x.wallpaperId = w.id))}></button>
    {/each}
  </div>
  <div class="actions">
    <label class="file"><Icon name="plus" /><span>Upload a picture</span><input type="file" accept="image/*" onchange={upload} /></label>
    {#if removable}<Button variant="destructive" text="Remove this picture" onclick={remove} />{/if}
  </div>
  {#if A.wallpaper?.credit}<p class="note">Photo by <a href={A.wallpaper.credit.link} target="_blank" rel="noopener noreferrer">{A.wallpaper.credit.author}</a> on Unsplash</p>{/if}
  <form class="search" onsubmit={(e) => { e.preventDefault(); void fromLink(); }}>
    <label class="visually-hidden" for="u-link">Paste a picture link</label>
    <input id="u-link" type="url" use:textField={{ kind: "url" }} placeholder="Paste a picture link" bind:value={link} autocomplete="off" />
    <Button text="Use link" type="submit" disabled={busy === "link" || !link.trim()} />
  </form>
  <form class="search" onsubmit={(e) => { e.preventDefault(); void search(); }}>
    <label class="visually-hidden" for="u-q">Search Unsplash</label>
    <input id="u-q" type="search" use:textField={{ kind: "search" }} placeholder="Search Unsplash" bind:value={query} autocomplete="off" />
    <Button text="Search" type="submit" disabled={busy === "search"} />
  </form>
  {#if results.length}
    <ul class="grid" aria-label="Unsplash results">
      {#each results as r (r.id)}
        <li><button type="button" class="tile" style:background-image="url({r.thumb})" aria-label="Use: {r.description}, by {r.credit.author}" disabled={busy === r.id} onclick={() => use(r)}></button></li>
      {/each}
    </ul>
    <p class="note">Results are generated stand-ins; the server will search Unsplash and store the photo you pick.</p>
  {/if}
</section>

{#if error}<p class="err" role="alert">{error}</p>{/if}
</div>

<style>
  .wp { display: grid; gap: var(--s-5); }
  section { display: grid; gap: var(--s-4); }
  h2 { font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-3); }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(5.5rem, 1fr)); gap: var(--s-3); }
  .grid li .tile { width: 100%; }
  .tile { aspect-ratio: 8 / 5; border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.08) center / cover no-repeat; display: grid; place-items: center; color: var(--ink-3); transition: scale var(--dur-fast) var(--ease-out); }
  .tile:active { scale: var(--press-scale); }
  .tile[aria-checked="true"] { outline: 2px solid var(--accent); outline-offset: 2px; }
  .actions, .search { display: flex; flex-wrap: wrap; gap: var(--s-3); align-items: center; }
  .file { position: relative; display: inline-flex; align-items: center; gap: var(--s-3); min-height: var(--pill-h); padding: 0 var(--s-6); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); font-weight: var(--strong-weight); }
  .file input { position: absolute; inset: 0; opacity: 0; cursor: pointer; }
  .file:focus-within { outline: 2px solid var(--accent); outline-offset: 2px; }
  .field, .search input { min-height: var(--pill-h); padding: 0 var(--s-5); border: 0; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); width: 100%; }
  .search input { flex: 1 1 7rem; width: auto; }
  .store { display: grid; gap: var(--s-1); }
  .store li { display: flex; align-items: center; gap: var(--s-4); min-height: var(--pill-h); }
  .sn { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tot { border-top: 1px solid var(--line); font-weight: var(--strong-weight); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
  .note a { color: var(--accent); }
  .err { color: var(--danger); font-size: var(--text-label); }
  .field-l { display: grid; gap: var(--s-2); font-weight: var(--strong-weight); font-size: var(--text-label); }
</style>
