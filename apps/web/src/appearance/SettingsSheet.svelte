<script lang="ts">
  // Appearance settings in a sheet (a side panel on a desktop, a bottom sheet on a phone): wallpaper (built-in, own upload, Unsplash search),
  // glass strengths, colour scheme and accent, and a font per role (interface, chat, code) from Google Fonts, Fontsource, Fontshare, the person's own
  // files or the system. Everything goes through the adapters in library.ts and fonts.ts, so the real server plugs in without changes here.
  import { Button, Capsule, Icon, IconButton, Row, Sheet, Switch, textField, tip } from "@calternal/ui";
  import PresetGallery from "./PresetGallery.svelte";
  import Slider from "./Slider.svelte";
  import { SOURCE_LABEL, type FontInfo, type FontRole, type FontSource } from "./fonts";
  import type { StockResult } from "./library";
  import { appearance as A } from "./prefs.svelte";
  import { shell } from "../shell/shell.svelte";

  // `inline`: the content as a page of the Settings app (a desktop has no sheet for settings, docs/settings.md); otherwise the bottom sheet a phone uses.
  let { open, onclose, inline = false }: { open: boolean; onclose: () => void; inline?: boolean } = $props();
  const p = $derived(A.prefs);
  $effect(() => { if (open || inline) void A.ensureThumbs(); });
  let view = $state<"main" | FontRole>("main");
  let query = $state("");
  let results = $state<StockResult[]>([]);
  let busy = $state("");
  let error = $state("");

  // ---- wallpaper ----
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
    if (!open && !inline) return;
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

  // ---- fonts ----
  let source = $state<FontSource>("google");
  let fq = $state("");
  let fonts = $state<FontInfo[]>([]);
  let ownName = $state("");
  const ROLE: Record<FontRole, string> = { ui: "Interface", chat: "Chat", code: "Code", brand: "Wordmark" };
  $effect(() => {
    if (view === "main") return;
    const s = source, q = fq, mono = view === "code";
    void A.fonts.search(s, q, mono).then((r) => (fonts = r));
  });
  async function pick(role: FontRole, f: FontInfo) {
    error = "";
    try {
      await A.fonts.install(f);
      A.change((x) => (x.fonts = { ...x.fonts, [role]: { family: f.family, source: f.source } }));
    } catch (err) {
      error = (err as Error).message;
    }
  }
  async function addOwn(e: Event) {
    e.preventDefault();
    const input = (e.currentTarget as HTMLFormElement).elements.namedItem("files") as HTMLInputElement;
    error = "";
    try {
      const info = await A.fonts.addOwn(ownName, [...(input.files ?? [])]);
      ownName = "";
      input.value = "";
      fonts = await A.fonts.search("upload", "", false);
      if (view !== "main") await pick(view, info);
    } catch (err) {
      error = (err as Error).message;
    }
  }
  const label = (r: FontRole) => `${A.prefs.fonts[r].family} · ${SOURCE_LABEL[A.prefs.fonts[r].source]}`;
  function close() {
    view = "main";
    error = "";
    onclose();
  }
</script>

{#snippet body()}
  <div class="in" class:inline>
    {#if view === "main"}
      <header><h1>Wallpaper and fonts</h1><IconButton icon="close" label={inline ? "Back to Appearance" : "Close settings"} onclick={close} /></header>

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

      <section aria-labelledby="s-theme">
        <h2 id="s-theme">Themes</h2>
        <PresetGallery />
        <p class="note">A preset sets the backdrop and the palette together. Each can still be changed on its own below.</p>
      </section>

      <section aria-labelledby="s-wall">
        <h2 id="s-wall">Wallpaper</h2>
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

      <section aria-labelledby="s-glass">
        <h2 id="s-glass">Materials</h2>
        {#each ([["blur", "Blur", "Frosts the picture behind the sidebars and the card."], ["dim", "Dim", "Darkens the picture under the card so text stays calm."], ["tint", "Tint", "Washes the surfaces with the colour scheme."]] as const) as [k, name, hint]}
          <Switch label={name} {hint} checked={p.materials[k]} onchange={(v) => A.change((x) => (x.materials[k] = v))} />
        {/each}
        <p class="note">How strong each one is comes from the picture itself: where it is too bright or too dark for text, the wash grows until the text passes contrast, whatever is ticked.</p>
      </section>

      <section aria-labelledby="s-size">
        <h2 id="s-size">Size</h2>
        <Slider label="UI size" unit=" %" min={80} max={150} value={p.uiSize} onchange={(v) => A.change((x) => (x.uiSize = v))} />
        <p class="note">One scale for type, spacing and controls together.</p>
      </section>

      <section aria-labelledby="s-switch">
        <h2 id="s-switch">App switcher (trying it out)</h2>
        <Capsule label="App switcher place" kind="radio" glass={false} items={[{ id: "sidebar", label: "In the sidebar" }, { id: "card", label: "Under the middle card" }]} value={p.switcher} onchange={(v) => A.change((x) => (x.switcher = v as never))} />
        {#if p.switcher === "card"}<Switch label="Hide until needed" hint="Off: always visible. On: it appears when the pointer or focus reaches the card's bottom edge." checked={p.switcherAutoHide} onchange={(v) => A.change((x) => (x.switcherAutoHide = v))} />{/if}
      </section>

      <section aria-labelledby="s-col">
        <h2 id="s-col">Colour</h2>
        <Capsule label="Colour scheme" kind="radio" glass={false} items={[{ id: "system", label: "System", icon: "apps" }, { id: "light", label: "Light", icon: "sun" }, { id: "dark", label: "Dark", icon: "moon" }]} value={A.scheme} onchange={(v) => A.setScheme(v as never)} />
        <Capsule label="Accent colour" kind="radio" glass={false} items={[{ id: "wallpaper", label: "From wallpaper" }, { id: "custom", label: "Custom" }]} value={p.accent} onchange={(v) => A.change((x) => (x.accent = v as never))} />
        {#if p.accent === "custom"}<label class="color"><span>Accent colour</span><input type="color" value={p.accentColor} oninput={(e) => A.change((x) => (x.accentColor = e.currentTarget.value))} /></label>{/if}
      </section>

      <section aria-labelledby="s-beh">
        <h2 id="s-beh">Behaviour</h2>
        <Switch label="Stream messages as they are written" hint="Off: a working state and the steps show, then the finished message appears at once." checked={p.streamText} onchange={(v) => A.change((x) => (x.streamText = v))} />
      </section>

      <section aria-labelledby="s-mark">
        <h2 id="s-mark">Wordmark</h2>
        <label class="field-l"><span>Name at the top of the sidebar</span><input class="field" type="text" use:textField value={p.brand} maxlength="24" autocomplete="off" oninput={(e) => A.change((x) => (x.brand = e.currentTarget.value))} /></label>
        <Row tall title="Wordmark font" sub={label("brand")} icon="notes" onclick={() => { view = "brand"; source = A.prefs.fonts.brand.source; fq = ""; error = ""; }}>
          {#snippet trailing()}<Icon name="chevron" inline />{/snippet}
        </Row>
      </section>

      <section aria-labelledby="s-font">
        <h2 id="s-font">Fonts</h2>
        {#each (["ui", "chat", "code"] as const) as r}
          <Row tall title={ROLE[r]} sub={label(r)} icon="notes" onclick={() => { view = r; source = A.prefs.fonts[r].source; fq = ""; error = ""; }}>
            {#snippet trailing()}<Icon name="chevron" inline />{/snippet}
          </Row>
        {/each}
        <p class="note">Google Sans Flex and Maple Mono are the defaults. A font from a catalogue is downloaded and stored by the server once, so pages never load from those sites.</p>
      </section>

      <footer><Button variant="quiet" text="Restore defaults" onclick={() => A.reset()} /></footer>
    {:else}
      <header><IconButton icon="back" label="Back to appearance settings" onclick={() => { view = "main"; error = ""; }} /><h1>{ROLE[view]} font</h1><span></span></header>
      <Capsule class="sources" label="Font source" kind="radio" glass={false} items={(["google", "fontsource", "fontshare", "upload", "system"] as const).map((s) => ({ id: s, label: SOURCE_LABEL[s] }))} value={source} onchange={(v) => { source = v as FontSource; fq = ""; error = ""; }} />
      {#if source !== "system" && source !== "upload"}
        <label class="visually-hidden" for="f-q">Search fonts</label>
        <input id="f-q" class="field" type="search" use:textField={{ kind: "search" }} placeholder="Search {SOURCE_LABEL[source]}" bind:value={fq} autocomplete="off" />
      {/if}
      <div role="radiogroup" aria-label="{ROLE[view]} font" class="fonts">
        {#each fonts as f (f.source + f.family)}
          {@const on = A.prefs.fonts[view].family === f.family && A.prefs.fonts[view].source === f.source}
          <button type="button" role="radio" aria-checked={on} class="frow" onclick={() => pick(view as FontRole, f)}>
            <span class="fam" style:font-family={f.ready ? (f.source === "system" ? "inherit" : `"${f.family}"`) : "inherit"}>{f.family}</span>
            <span class="src">{f.ready ? SOURCE_LABEL[f.source] : "needs the server"}</span>
            {#if on}<Icon name="check" inline />{/if}
          </button>
        {:else}
          <p class="note">{source === "upload" ? "No fonts of your own yet." : "Nothing matches."}</p>
        {/each}
      </div>
      {#if source === "upload"}
        <form class="own" onsubmit={addOwn}>
          <label class="field-l"><span>Name</span><input class="field" type="text" use:textField bind:value={ownName} autocomplete="off" /></label>
          <label class="field-l"><span>Font files (woff2, woff, ttf, otf)</span><input class="field" name="files" type="file" multiple accept=".woff2,.woff,.ttf,.otf" /></label>
          <Button text="Add this font" type="submit" />
        </form>
        <p class="note">Use only fonts you are licensed to use. Several files under one name become its weights and italics, read from the file names.</p>
      {/if}
    {/if}
    {#if error}<p class="err" role="alert">{error}</p>{/if}
  </div>
{/snippet}

{#if inline}
  <div class="inline-page">{@render body()}</div>
{:else}
  <Sheet {open} label="Appearance settings" onclose={close} side="bottom">{@render body()}</Sheet>
{/if}

<style>
  .in { display: grid; gap: var(--s-5); padding: var(--s-6); }
  .in.inline { padding: 0; }
  .inline-page { box-sizing: border-box; min-width: 0; width: 100%; max-width: 35rem; margin: 0 auto; padding: calc(var(--safe-top) + var(--bar-h) + var(--s-6)) var(--s-6) var(--s-10); }
  header { display: flex; align-items: center; justify-content: space-between; gap: var(--s-4); min-height: var(--bar-h); }
  h1 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  h2 { font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-3); }
  section { display: grid; gap: var(--s-4); }
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
  :global(.sources) { --seg-pad: var(--s-2); }
  .store { display: grid; gap: var(--s-1); }
  .store li { display: flex; align-items: center; gap: var(--s-4); min-height: var(--pill-h); }
  .sn { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tot { border-top: 1px solid var(--line); font-weight: var(--strong-weight); }
  .color { display: flex; align-items: center; justify-content: space-between; font-weight: var(--strong-weight); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
  .note a { color: var(--accent); }
  .err { color: var(--danger); font-size: var(--text-label); }
  footer { display: flex; justify-content: flex-end; }
  .fonts { display: grid; gap: var(--s-1); }
  .frow { display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h-settings); padding: 0 var(--s-5); border-radius: var(--r-select); text-align: start; }
  .frow[aria-checked="true"] { background: var(--accent-soft); color: var(--accent); }
  @media (hover: hover) and (pointer: fine) { .frow:not([aria-checked="true"]):hover { background: var(--hover); } }
  .fam { flex: 1; font-size: var(--text-heading); }
  .src { font-size: var(--text-label); color: var(--ink-3); }
  .own { display: grid; gap: var(--s-4); }
  .field-l { display: grid; gap: var(--s-2); font-weight: var(--strong-weight); font-size: var(--text-label); }
</style>
