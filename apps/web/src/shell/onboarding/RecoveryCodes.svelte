<script lang="ts">
  // A set of recovery codes, shown once: ten codes of 32 hex characters in a grid of readable groups (four groups of eight), with Copy and Download (one code per line, exactly as issued) and an explicit "I have saved them".
  // The codes live only in this component's props: they are never stored, logged or put in an address. Used by the onboarding and by Settings, Account when a new set is made.
  import { Button } from "@calternal/ui";
  import { codesText, groupCode } from "./account-rules";
  let { codes, onsaved, label = "I have saved them" }: { codes: string[]; onsaved: () => void; label?: string } = $props();
  let copied = $state(false);
  async function copy() { try { await navigator.clipboard.writeText(codesText(codes)); copied = true; setTimeout(() => (copied = false), 2000); } catch { /* the codes are on screen: select them */ } }
  function download() {
    const url = URL.createObjectURL(new Blob([codesText(codes)], { type: "text/plain" }));
    const a = Object.assign(document.createElement("a"), { href: url, download: "calternal-recovery-codes.txt" });
    document.body.append(a); a.click(); a.remove(); setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
</script>

<div class="rc">
  <ol class="codes" aria-label="Recovery codes">{#each codes as c (c)}<li><code>{groupCode(c)}</code></li>{/each}</ol>
  <div class="act"><Button class="settings-action" variant="secondary" text={copied ? "Copied" : "Copy"} onclick={copy} /><Button class="settings-action" variant="secondary" text="Download" onclick={download} /></div>
  <p class="note">Each code works once, and they are shown only now. Keep them somewhere safe, apart from this device.</p>
  <div class="go"><Button variant="primary" text={label} onclick={onsaved} /></div>
</div>

<style>
  .rc { display: grid; gap: var(--s-5); }
  .codes { list-style: none; padding: var(--s-4) var(--s-5); margin: 0; display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--s-3) var(--s-6); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.07); }
  code { font-family: var(--font-mono); font-size: var(--text-label); overflow-wrap: anywhere; }
  .act { display: flex; gap: var(--s-4); flex-wrap: wrap; }
  .note { font-size: var(--text-label); color: var(--ink-3); }
  .go { display: flex; }
  @container settings-page (max-width: 40rem) { .codes { grid-template-columns: minmax(0, 1fr); } }
  @media (max-width: 520px) { .codes { grid-template-columns: minmax(0, 1fr); } .act :global(.settings-action) { flex: 1 1 100%; width: 100%; } }
</style>
