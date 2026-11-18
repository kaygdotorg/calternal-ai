<script lang="ts">
  // The list of every command with its chord, grouped, with a plain description of where it applies: Change then the new keys (Esc cancels); a chord already in use says so and is refused.
  // Shared by the quick sheet (Cmd or Ctrl + / or ?) and the Keyboard page of Settings, so the two cannot differ. `file` adds the account's shortcuts file, as it would be stored.
  import { Button } from "@calternal/ui";
  import { show, shortcuts as S } from "../lib/shortcuts.svelte";
  let { file = false }: { file?: boolean } = $props();
  const groups = ["Navigation", "Threads", "Canvas", "Help"] as const;
  const FIXED: [string, string][] = [["Send the message", "Enter"], ["New line in the message", "Shift+Enter"], ["Move the focused thread within its section", "Alt+↑ or Alt+↓"], ["Move it to the next or previous section", "Alt+Shift+↑ or ↓"], ["Rename the focused thread or section", "F2"], ["Open the context menu on a thread", "Menu key or Shift+F10"], ["Close a menu or sheet", "Esc"]];
  /** The per-account shortcuts file: a Markdown file of `command: chord` lines, only what differs from the defaults (the server keeps it per account). */
  const asFile = $derived(["---", "kind: shortcuts", "---", "", ...Object.entries(S.overrides).map(([id, c]) => `${id}: ${c ?? "none"}`)].join("\n") + "\n");
</script>

<div class="sc">
    {#if S.problem}<p class="err" role="alert">{S.problem}</p>{/if}
    {#each groups as g}
      {@const cs = S.commands.filter((c) => c.group === g)}
      {#if cs.length}
        <section aria-labelledby="sc-{g}">
          <h2 id="sc-{g}">{g}</h2>
          <ul>
            {#each cs as c (c.id)}
              <li>
                <span class="tx"><span>{c.label}</span><small>{c.where}</small></span>
                <kbd class:wait={S.capturing === c.id}>{S.capturing === c.id ? "Press the keys…" : show(S.chordFor(c.id))}</kbd>
                <Button variant="quiet" text={S.capturing === c.id ? "Cancel" : "Change"} onclick={() => { S.problem = ""; S.capturing = S.capturing === c.id ? null : c.id; }} />
                {#if c.id in S.overrides}<Button variant="quiet" text="Reset" onclick={() => S.reset(c.id)} />{/if}
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    {/each}
    <section aria-labelledby="sc-fixed">
      <h2 id="sc-fixed">Fixed</h2>
      <ul>{#each FIXED as [label, keys]}<li><span class="tx"><span>{label}</span></span><kbd>{keys}</kbd></li>{/each}</ul>
    </section>
    {#if file}
      <section aria-labelledby="sc-file">
        <h2 id="sc-file">Your shortcuts file</h2>
        <pre class="file" id="sc-file-text">{asFile}</pre>
        <div class="act"><Button variant="quiet" text="Copy the file" onclick={() => void navigator.clipboard?.writeText(asFile)} /><Button variant="quiet" text="Reset all" onclick={() => S.resetAll()} /></div>
      </section>
    {/if}
</div>

<style>
  .sc { display: grid; gap: var(--s-5); }
  .file { margin: 0; padding: var(--s-4) var(--s-5); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.06); font-family: var(--font-mono); font-size: var(--text-code); white-space: pre-wrap; }
  .act { display: flex; gap: var(--s-3); margin-top: var(--s-3); }
  h2 { font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-3); margin-bottom: var(--s-2); }
  ul { display: grid; gap: var(--s-1); }
  @media (max-width: 600px) { li { flex-wrap: wrap; } .tx { flex-basis: 100%; } }
  li { display: flex; align-items: center; gap: var(--s-3); min-height: var(--row-h-settings); padding: 0 var(--s-2) 0 var(--s-4); border-radius: var(--r-select); }
  .tx { flex: 1; min-width: 0; display: grid; }
  small { font-size: var(--text-label); color: var(--ink-3); }
  kbd { font-family: var(--font-ui); font-size: var(--text-label); padding: var(--s-1) var(--s-4); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.08); white-space: nowrap; }
  kbd.wait { background: var(--accent-soft); color: var(--accent); }
  .err { color: var(--danger); font-size: var(--text-label); }
</style>
