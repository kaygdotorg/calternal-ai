<script lang="ts">
  // The account sheet: who is signed in and the colour scheme. Fixture identity; sign-in and passkeys are the server's.
  import { Capsule, Icon, Row, Sheet, applyScheme, schemePreference, type SchemePreference } from "@calternal/ui";
  let { open, onclose }: { open: boolean; onclose: () => void } = $props();
  let pref = $state<string>(schemePreference());
</script>

<Sheet {open} label="Account" {onclose}>
  <div class="in">
    <header><span class="av" aria-hidden="true">K</span><div><h2>Kay</h2><p>Personal space</p></div></header>
    <h3 id="scheme">Appearance</h3>
    <Capsule label="Colour scheme" kind="radio" glass={false} items={[{ id: "system", label: "System", icon: "apps" }, { id: "light", label: "Light", icon: "sun" }, { id: "dark", label: "Dark", icon: "moon" }]} bind:value={pref} onchange={(v) => applyScheme(v as SchemePreference)} />
    <Row title="Settings" icon="settings" href="/settings/account" onclick={onclose} />
  </div>
</Sheet>

<style>
  .in { display: grid; gap: var(--s-5); padding: var(--s-6); }
  header { display: flex; align-items: center; gap: var(--s-5); }
  .av { width: var(--s-10); height: var(--s-10); border-radius: 50%; display: grid; place-items: center; background: var(--accent-soft); color: var(--accent); font-family: var(--font-rounded); font-size: var(--text-heading); font-weight: var(--strong-weight); }
  h2 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  p { color: var(--ink-3); font-size: var(--text-label); }
  h3 { font-size: var(--text-section); font-weight: var(--strong-weight); letter-spacing: 0.05em; text-transform: uppercase; color: var(--ink-3); }
</style>
