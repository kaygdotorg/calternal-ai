<script lang="ts">
  // One action as a control: an invoke (resolved and dispatched by the host's rules) or a shell action. The label, icon, destructive role and tone are the description's; whether it needs a sheet, a challenge or one
  // exchange is the host's. While it runs the button is aria-disabled (it keeps focus) and a second activation is ignored. `context` is the host-issued activation context for exactly what this control was drawn for;
  // without one nothing is sent.
  import { Button } from "@calternal/ui";
  import type { Action, Interaction } from "../model";
  import { useActions } from "../invoke/context";
  import { knownIcon } from "./icons";
  import { shellAllowed } from "../invoke/shell";
  import type { Selection } from "../windows/selection.svelte";

  let { action, context, interaction, selection, setOf, style = "quiet", disabled = false, onrun }: {
    action: Action; context: string | undefined; interaction?: () => Pick<Interaction, "form" | "draft" | "selection_ref">;
    /** When the action is bound to the selection, the held selection is flushed first and the context re-read from it. */
    selection?: Selection; /** The bind whose ticked rows this action takes as a host-issued selection reference. */ setOf?: string; style?: "primary" | "quiet"; disabled?: boolean; onrun?: (outcome: unknown) => void;
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
      const out = await ui.invoker.run(inv, { context, interaction: inter, flush: selection && !setOf ? () => selection.flush() : undefined, recontext: selection && !setOf ? () => selection.contextFor(inv.id) : undefined });
      onrun?.(out);
    } finally { busy = false; }
  }
</script>

<Button text={title} icon={typeof action.icon === "string" ? knownIcon(action.icon) : undefined} variant={action.role === "destructive" ? "destructive" : style} {busy} aria-disabled={busy || disabled || (!!inv && !context && !selection) || undefined} data-action={action.id} data-tone={"tone" in action ? action.tone : undefined} onclick={activate} />
