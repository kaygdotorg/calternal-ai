<script lang="ts">
  // Shown to a signed-in person whose server now requires a verified email and who has none (`email_setup_required`): an address and its code before going on. Reloading brings the app in once it is confirmed.
  import { Card } from "@calternal/ui";
  import EmailForm from "./onboarding/EmailForm.svelte";
  import Backdrop from "./Backdrop.svelte";
  let { mail = true }: { mail?: boolean } = $props();
</script>
<Backdrop />
<main class="wrap">
  <p class="mark" aria-hidden="true">Calternal</p>
  <Card label="Add your email" variant="solid" class="card">
    <div class="in"><h1>Add your email address</h1><p class="note">This server needs a verified email for every account. Add yours to go on.</p><EmailForm {mail} ondone={() => location.reload()} /></div>
  </Card>
</main>
<style>
  .wrap { position: relative; z-index: 1; min-height: 100dvh; display: grid; align-content: center; justify-items: center; gap: var(--s-6); padding: calc(var(--safe-top) + var(--s-6)) calc(var(--safe-right) + var(--s-6)) calc(var(--safe-bottom) + var(--s-6)) calc(var(--safe-left) + var(--s-6)); box-sizing: border-box; }
  /* The script face's last "l" swings past its advance width; the box is padded so the whole stroke is inside it. Without the padding Safari paints the overhang as a separate piece while the word moves (a stray comma-like mark after the wordmark when the page re-centres). */
  .mark { font-family: var(--font-brand, inherit); font-size: var(--text-title); font-weight: var(--strong-weight); color: var(--ink); padding-inline: 0.3em; }
  .wrap :global(.card) { width: min(30rem, 100%); }
  .in { display: grid; gap: var(--s-5); padding: var(--s-7); }
  h1 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
</style>
