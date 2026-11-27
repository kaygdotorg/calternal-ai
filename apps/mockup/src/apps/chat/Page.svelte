<script lang="ts">
  // The Chat app's window: the conversation of the thread chosen in the sidebar (a thread opened here stays in this window).
  import Conversation from "./Conversation.svelte";
  import { ITEMS } from "../../data/collections";
  import type { PageProps } from "../registry";
  let { win }: PageProps = $props();
</script>

<Conversation item={ITEMS[win.sel ?? "c-audit"]} />
