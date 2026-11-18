<script lang="ts">
  // Fonts, as a section of Settings, Appearance: a family per role (interface, chat, code, and the wordmark) from Google Fonts, Fontsource, Fontshare, the person's own files or the system. Wide: the roles on the left and the chosen role's
  // picker beside them (no nested page); narrow: the list, then the picker with a Back button. Everything goes through the adapters in fonts.ts, so the real server plugs in without changes here.
  import { Button, Capsule, Icon, Row, SplitView, textField } from "@calternal/ui";
  import { SOURCE_LABEL, type FontInfo, type FontRole, type FontSource } from "./fonts";
  import { appearance as A } from "./prefs.svelte";
  let error = $state("");
  let source = $state<FontSource>("google");
  let fq = $state("");
  let fonts = $state<FontInfo[]>([]);
  let ownName = $state("");
  const ROLE: Record<FontRole, string> = { ui: "Interface", chat: "Chat", code: "Code", brand: "Wordmark" };
  const ROLES = ["ui", "chat", "code", "brand"] as const;
  /** The role whose picker is open (the list on the left stays: no nested page when the card is wide). */
  let view = $state<FontRole | null>(null);
  $effect(() => {
    if (!view) return;
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
      if (view) await pick(view, info);
    } catch (err) {
      error = (err as Error).message;
    }
  }
  const label = (r: FontRole) => `${A.prefs.fonts[r].family} · ${SOURCE_LABEL[A.prefs.fonts[r].source]}`;
  function choose(r: FontRole | null) { view = r; error = ""; fq = ""; if (r) source = A.prefs.fonts[r].source; }
</script>

<div class="fp">
  <SplitView selected={view !== null} onback={() => choose(null)} listLabel="Font roles" breakpoint="30rem" listWidth="13rem">
    {#snippet list()}
      {#each ROLES as r (r)}
        <Row tall title={ROLE[r]} sub={label(r)} selected={view === r} onclick={() => choose(r)}>{#snippet trailing()}<Icon name="chevron" inline />{/snippet}</Row>
      {/each}
      <p class="note">Google Sans Flex and Maple Mono are the defaults. A font from a catalogue is downloaded and stored by the server once, so pages never load from those sites.</p>
    {/snippet}
    {#snippet detail()}
      {#if view}
        <h2>{ROLE[view]} font</h2>
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
        {#if error}<p class="err" role="alert">{error}</p>{/if}
      {:else}
        <p class="note">Choose a role to pick its font.</p>
      {/if}
    {/snippet}
  </SplitView>
</div>

<style>
  .fp { min-height: 22rem; }
  h2 { font-size: var(--text-heading); font-weight: var(--strong-weight); margin-bottom: var(--s-4); }
  .note { font-size: var(--text-label); color: var(--ink-3); margin-top: var(--s-3); }
  .err { color: var(--danger); font-size: var(--text-label); }
  .fonts { display: grid; gap: var(--s-1); margin-top: var(--s-4); }
  .frow { display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h-settings); padding: 0 var(--s-5); border-radius: var(--r-select); text-align: start; }
  .frow[aria-checked="true"] { background: var(--accent-soft); color: var(--accent); }
  @media (hover: hover) and (pointer: fine) { .frow:not([aria-checked="true"]):hover { background: var(--hover); } }
  .fam { flex: 1; font-size: var(--text-heading); }
  .src { font-size: var(--text-label); color: var(--ink-3); }
  .field { min-height: var(--pill-h); padding: 0 var(--s-5); border: 0; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); width: 100%; margin-top: var(--s-4); }
  .own { display: grid; gap: var(--s-4); margin-top: var(--s-4); }
  .field-l { display: grid; gap: var(--s-2); font-weight: var(--strong-weight); font-size: var(--text-label); }
  :global(.sources) { --seg-pad: var(--s-2); }
</style>
