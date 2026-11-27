<script lang="ts">
  // A note as a page (one component: a window's body on a desktop, the whole screen on a phone). Order: the cover (if it has one), the title, one quiet line (folder, tags, who is on it, when it was edited), the body, and at the foot the notes that link here and the tasks that came out of it.
  // States it can be in: a normal page; EMPTY (a new note: the placeholder and the caret in the first line); LONG (it scrolls, the outline in the side panel follows it); a note the shell made up (a short stand-in). A fixture: nothing here is edited.
  // The text is at the interface's own sizes (title 22 px on a desktop, body 13), a column measured in characters (60ch standard, 58ch reading: about 70 and 66 characters of running text), so the line stays comfortable whatever the size.
  import Blocks from "./Blocks.svelte";
  import NoteLook from "./NoteLook.svelte";
  import Glyph from "../../shell/AppIcon.svelte";
  import TaskRow from "../tasks/TaskRow.svelte";
  import Av from "../../shell/Av.svelte";
  import { docOf, linksTo, wordCount } from "./data";
  import { tasks, xOf } from "../tasks/data";
  import { wins } from "../../windows.svelte";
  import { notesUi } from "./ui.svelte";
  import type { Item } from "../../data/collections";
  let { item }: { item: Item } = $props();
  const doc = $derived(docOf(item)), back = $derived(linksTo(item.id)), made = $derived(tasks.filter((t) => xOf(t.id).note === item.id)), empty = $derived(doc.blocks.length === 0);
</script>

<div class="np" class:reading={notesUi.reading} data-note-page={item.id}>
  <article>
    {#if doc.cover !== undefined}<div class="cover" style:--h={doc.cover} aria-hidden="true"></div>{/if}
    <header>
      <h1 class:ph={empty}>{item.title}</h1>
      <p class="meta"><span class="fd"><Glyph name="folder" inline />{doc.folder}</span>{#each doc.tags as t (t)}<span class="tg">#{t}</span>{/each}
        <span class="sp"></span>{#if doc.shared.length > 1}<span class="who" aria-label={`Shared with ${doc.shared.length - 1} people`}>{#each doc.shared.slice(0, 3) as p (p.name)}<Av name={p.name} size="28px" />{/each}</span>{/if}
        <span>{empty ? "Not saved yet" : `Edited ${doc.edited} · ${wordCount(item.id)} words`}</span>
        <button type="button" class="aa" aria-pressed={notesUi.reading} aria-label={notesUi.reading ? "Text size: reading (larger)" : "Text size: standard"} title={notesUi.reading ? "Reading size (click for standard)" : "Standard size (click for reading)"} onclick={() => notesUi.set(!notesUi.reading)}>Aa</button></p>
    </header>
    {#if empty}<div class="start"><p><span class="caret" aria-hidden="true"></span><span class="hint">Start writing, or press / for blocks</span></p></div>
    {:else}<Blocks blocks={doc.blocks} id={item.id} />{/if}
    <footer>
      <section aria-labelledby="bl-h"><h3 id="bl-h">Linked from <span>{back.length}</span></h3>
        {#each back as n (n.id)}<NoteLook item={n} onopen={(i) => wins.openItem(i)} />{:else}<p class="none">No other note links here yet. Type <code>[[</code> in a note to link this one.</p>{/each}</section>
      {#if made.length}<section aria-labelledby="mt-h"><h3 id="mt-h">Tasks from this note <span>{made.length}</span></h3>{#each made as t (t.id)}<TaskRow item={t} project={false} onopen={(i) => wins.openItem(i)} />{/each}</section>{/if}
    </footer>
  </article>
</div>

<style>
  /* the page: STANDARD is the interface's body size (13 px on a desktop); READING is one step larger (the heading token), a longer line height, more air between blocks, and a slightly narrower measure in characters. Everything in the body follows --body. */
  .np { --body: var(--text-ui); --h2: var(--text-heading); --code: var(--text-label); --quote: var(--text-heading); --lh: 1.6; --gap: var(--s-5); --measure: 60ch; flex: 1; width: 100%; min-height: 0; overflow: auto; container-type: inline-size; scrollbar-gutter: stable; } .np.reading { --body: var(--text-heading); --h2: calc(var(--text-heading) * 1.2); --code: calc(var(--text-label) * 1.15); --quote: calc(var(--text-heading) * 1.15); --lh: 1.72; --gap: var(--s-6); --measure: 58ch; }
  article { font-size: var(--body); max-width: var(--measure); margin-inline: auto; padding: var(--s-2) var(--s-2) var(--s-10); }
  .cover { height: 7.5rem; margin-bottom: var(--s-6); border-radius: var(--r-menu); background: radial-gradient(50% 120% at 80% 0%, oklch(0.9 0.09 calc(var(--h) + 60) / 0.8), transparent 60%), linear-gradient(135deg, oklch(0.82 0.09 var(--h)), oklch(0.68 0.12 calc(var(--h) + 50))); }
  header { display: grid; gap: var(--s-4); margin-bottom: var(--s-7); } h1 { margin: 0; font-size: var(--text-title); line-height: 1.2; font-weight: var(--strong-weight); text-wrap: balance; } h1.ph { color: var(--ink-4); }
  .meta { margin: 0; display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-3) var(--s-4); color: var(--ink-3); font-size: var(--text-label); line-height: 1.3; } .sp { flex: 1; min-width: var(--s-3); }
  .fd { display: inline-flex; align-items: center; gap: var(--s-2); } .tg { padding: 0 var(--s-4); min-height: var(--s-7); display: inline-flex; align-items: center; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink-2); font-size: var(--text-label); line-height: 1.25; } .who { display: inline-flex; } .who :global(.in) { font-size: var(--text-section, 11px); letter-spacing: 0; } /* two initials at the smallest type size (11 px) sit inside a 28 px circle */ .who :global(.av) { margin-inline-start: -4px; box-shadow: 0 0 0 2px var(--paper, Canvas), inset 0 0 0 1px rgb(var(--ink-rgb) / 0.08); } .who :global(.av:first-child) { margin-inline-start: 0; }
  .start p { margin: 0; font-size: var(--body); line-height: var(--lh); color: var(--ink-4); } .caret { display: inline-block; width: 2px; height: 1.15em; margin-inline-end: var(--s-1); vertical-align: text-bottom; border-radius: 1px; background: var(--accent-base); animation: blink var(--dur-pulse) steps(1) infinite; } .hint { color: var(--ink-3); } @keyframes blink { 50% { opacity: 0; } } @media (prefers-reduced-motion: reduce) { .caret { animation: none; } }
  .aa { padding: 0 var(--s-3); border-radius: var(--r-select); color: var(--ink-3); font-size: var(--text-label); font-weight: var(--strong-weight); line-height: var(--s-8); transition: background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out); } .aa[aria-pressed="true"] { color: var(--accent); background: var(--accent-soft); } @media (hover: hover) and (pointer: fine) { .aa:hover { background: rgb(var(--ink-rgb) / 0.08); color: var(--ink); } }
  footer { display: grid; gap: var(--s-7); margin-inline: calc(var(--s-5) * -1) calc(var(--s-2) * -1); margin-top: var(--s-10); padding-top: var(--s-6); background: linear-gradient(var(--line), var(--line)) no-repeat var(--s-5) 0 / calc(100% - var(--s-5) * 2) 1px; }
  h3 { margin: 0 0 var(--s-2); padding-inline: var(--s-5); color: var(--ink-2); font-size: var(--text-label); font-weight: var(--strong-weight); } h3 span { color: var(--ink-3); font-weight: var(--ui-weight); font-variant-numeric: tabular-nums; margin-inline-start: var(--s-2); }
  .none { margin: 0; padding: 0 var(--s-5); color: var(--ink-3); font-size: var(--text-ui); line-height: 1.5; } code { font-family: var(--font-mono); font-size: 0.92em; padding: 0 var(--s-2); border-radius: var(--s-2); background: rgb(var(--ink-rgb) / 0.07); }
</style>
