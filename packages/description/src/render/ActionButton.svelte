<script lang="ts">
  // One action as a control: an invoke (resolved and dispatched by the host's rules) or a shell action. The label, icon, destructive role and tone are the description's; whether it needs a sheet, a challenge or one
  // exchange is the host's. While it runs the button is aria-disabled (it keeps focus) and a second activation is ignored. `context` is the host-issued activation context for exactly what this control was drawn for;
  // without one nothing is sent.
  import { Button, Icon, IconButton, Spinner } from "@calternal/ui";
  import type { Action, Interaction } from "../model";
  import { useActions } from "../invoke/context";
  import { knownIcon } from "./icons";
  import { shellAllowed } from "../invoke/shell";
  import type { Selection } from "../windows/selection.svelte";

  let { action, context, interaction, selection, setOf, style = "quiet", disabled = false, onrun, round = false, iconOnly = false, preset }: {
    action: Action; context: string | undefined; interaction?: () => Pick<Interaction, "form" | "draft" | "selection_ref">;
    /** When the action is bound to the selection, the held selection is flushed first and the context re-read from it. */
    selection?: Selection; /** The bind whose ticked rows this action takes as a host-issued selection reference. */ setOf?: string; style?: "primary" | "quiet"; disabled?: boolean; onrun?: (outcome: unknown) => void; /** A round, icon-only send control (the composer's), still named by the action's title. */ round?: boolean; /** Draw only the icon (a narrow header); the title stays the accessible name and tooltip. */ iconOnly?: boolean; /** Choices the person already made (chips) and the activation id they were fetched for. */ preset?: () => { activation_id?: string; harness?: string } | undefined;
  } = $props();
  const { ui, onshell } = useActions();
  let busy = $state(false);
  const title = $derived(action.title ?? "");
  const inv = $derived("operation" in action ? action : null);
  async function activate() {
    if (disabled || busy) return;
    if (!inv) { if (shellAllowed(action)) onshell?.(action); return; } // only the shell controls the contract allows a description to request
    busy = true;
    try {
      let inter = interaction?.();
      if (setOf && selection) { const ref = await selection.ref(); if (!ref) return; inter = { ...inter, selection_ref: ref }; } // the host validates the ticked rows and returns the reference; the action names it, never the ids
      const out = await ui.invoker.run(inv, { context, interaction: inter, ...(preset?.() ?? {}), flush: selection && !setOf ? () => selection.flush() : undefined, recontext: selection && !setOf ? () => selection.contextFor(inv.id) : undefined });
      onrun?.(out);
    } finally { busy = false; }
  }
</script>

{#if iconOnly && typeof action.icon === "string" && knownIcon(action.icon)}
  <IconButton icon={knownIcon(action.icon)!} label={title} data-action={action.id} aria-disabled={busy || disabled || (!!inv && !context && !selection) || undefined} onclick={activate} />
{:else if round}
  <button type="button" class="go" data-state={busy ? "busy" : disabled || (!!inv && !context && !selection) ? "idle" : "ready"} aria-label={title} title={title} aria-disabled={busy || disabled || (!!inv && !context && !selection) || undefined} data-action={action.id} onclick={activate}>{#if busy}<Spinner />{:else}<Icon name={(typeof action.icon === "string" ? knownIcon(action.icon) : undefined) ?? "send"} />{/if}</button>
{:else}<Button text={title} icon={typeof action.icon === "string" ? knownIcon(action.icon) : undefined} variant={action.role === "destructive" ? "destructive" : style} {busy} aria-disabled={busy || disabled || (!!inv && !context && !selection) || undefined} data-action={action.id} data-has-icon={typeof action.icon === "string" && !!knownIcon(action.icon) ? "" : undefined} title={title} data-tone={"tone" in action ? action.tone : undefined} onclick={activate} />
{/if}

<style>
  /* The send control of the composer (the web app's SendControl): a round pill-height button, the accent when there is something to send, quiet otherwise. */
  .go { flex: none; display: inline-grid; place-items: center; width: var(--pill-h); height: var(--pill-h); border-radius: 50%; transition: scale var(--dur-fast) var(--ease-out), background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out); }
  .go:active:not([aria-disabled="true"]) { scale: var(--press-scale); }
  .go[data-state="idle"], .go[data-state="busy"] { background: rgb(var(--ink-rgb) / 0.1); color: var(--ink-3); cursor: default; }
  .go[data-state="ready"] { background: var(--accent-base); color: #fff; }
</style>
