<script lang="ts">
  // The one search field: a pill with a search icon and a text field that starts at the start edge. It has the shared text behaviour (`textField`: undo, the shared shortcuts, no line breaks, spelling off). `label` is its accessible
  // name and, with no `placeholder`, its hint. A host that handles arrow keys or Enter (a result list under it) passes `onkeydown`.
  import Icon from "./Icon.svelte";
  import { textField } from "../lib/textfield";
  let { value = $bindable(""), label, placeholder, el = $bindable(), onkeydown, oninput, class: klass }: { value?: string; label: string; placeholder?: string; el?: HTMLInputElement; onkeydown?: (e: KeyboardEvent) => void; oninput?: () => void; class?: string } = $props();
</script>

<div class={["sf", klass]} data-ring-host>
  <Icon inline name="search" />
  <input bind:this={el} bind:value type="text" use:textField={{ kind: "search", cancels: true }} placeholder={placeholder ?? label} aria-label={label} autocomplete="off" {onkeydown} {oninput} />
</div>

<style>
  .sf { display: flex; align-items: center; gap: var(--s-3); padding: 0 var(--s-4); min-height: var(--pill-h); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink-3); }
  .sf input { flex: 1; min-width: 0; border: 0; background: none; color: var(--ink); font-size: var(--text-ui); text-align: start; }
  .sf input:focus, .sf input:focus-visible { outline: none; }
</style>
