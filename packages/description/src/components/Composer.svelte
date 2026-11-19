<script lang="ts">
  // The composer host component: the draft (text) lives ONLY on the device until Send; the option controls the config names (provider, model, reasoning, permission, machine) list what the host returns through
  // `composer.options` for this thread, and the renderer never lists an option the host did not return. The host owns the send. It is resolved ahead only when the prepared MATERIAL changes (an option), never for
  // text: Send is then one exchange, `invoke_confirm` carrying `final_text` (the composer-only arm), and the answer is the outcome. The draft clears only after `ok`; an error keeps it and shows the host's message.
  // Any parser-derived change the host reports (an @-mention, a slash command in the text) comes back as a fresh challenge, which is shown and resolved again before anything is sent.
  import type { ActionOutcome, ActivationResolution, ComposerOptionPage, Composer } from "../model";
  import { useApp, useActions } from "../invoke/context";
  import { resolveRef } from "./ref";

  let { config, id }: { config: Composer["config"]; id: string } = $props();
  const app = useApp(), { session } = useActions();
  const thread = $derived(resolveRef(config.thread, app));
  const OPTION_CONTROLS = ["provider", "model", "reasoning", "permission", "machine"] as const;
  const shown = $derived(OPTION_CONTROLS.filter((c) => (config.controls ?? []).includes(c)));
  let text = $state(""), pages = $state<Record<string, ComposerOptionPage>>({}), chosen = $state<Record<string, string>>({});
  let message = $state<string | null>(null), busy = $state(false);
  let prepared: { options: string; resolution: ActivationResolution; activation_id: string } | null = null;

  $effect(() => {
    const t = thread; pages = {}; chosen = {}; prepared = null;
    if (!t) return;
    for (const c of shown) void session.exchange<ComposerOptionPage>("composer.options", { thread: t, control: c }).then((p) => { pages[c] = p; if (p.selected ?? p.default) chosen[c] = (p.selected ?? p.default)!; }, () => {});
  });

  const newId = () => globalThis.crypto?.randomUUID?.() ?? `act-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  const options = () => ({ ...chosen });
  /** Resolves the prepared material (options and the thread, not the words). Done when the material changed since the last resolution. */
  async function prepare(): Promise<typeof prepared> {
    const key = JSON.stringify(options());
    if (prepared && prepared.options === key) return prepared;
    const activation_id = newId();
    const resolution = await session.exchange<ActivationResolution>("activation.resolve", { type: "invoke", action_id: id, activation_id, activation_context: session.viewContexts[id], interaction: { options: options() } });
    prepared = { options: key, resolution, activation_id };
    return prepared;
  }
  async function send() {
    if (!thread || busy || !text.trim()) return;
    busy = true; message = null;
    try {
      const p = await prepare();
      const r = p!.resolution;
      const out = await session.exchange<ActionOutcome>("activation.confirm", { type: "invoke_confirm", activation_id: p!.activation_id, resolution_ref: r.resolution_ref, confirmation_challenge: r.confirmation_challenge ?? "", final_text: text });
      if (out.state === "ok") { text = ""; prepared = null; }
      else { message = out.message ?? "The message was not sent."; if (out.state === "stale") prepared = null; }
    } catch { message = "The host did not answer, so it is not known whether the message was sent."; prepared = null; }
    finally { busy = false; }
  }
  const keydown = (e: KeyboardEvent) => { if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); void send(); } };
</script>

<form class="cp" data-host-component="composer" data-host-component-id={id} onsubmit={(e) => { e.preventDefault(); void send(); }}>
  {#if shown.length}
    <div class="opts">{#each shown as c (c)}{@const pg = pages[c]}
      {#if pg}<label class="o"><span class="l">{c}</span>
        <select data-control={c} value={chosen[c] ?? ""} onchange={(e) => { chosen[c] = e.currentTarget.value; }}>{#each pg.options as o (o.ref)}<option value={o.ref}>{o.label}</option>{/each}</select></label>{/if}
    {/each}</div>
  {/if}
  {#if message}<div class="err" role="alert" data-composer-error>{message}</div>{/if}
  <div class="row">
    <textarea bind:value={text} placeholder={config.placeholder ?? "Message"} aria-label={config.placeholder ?? "Message"} rows="2" maxlength="65536" onkeydown={keydown}></textarea>
    <button type="submit" class="send" aria-disabled={busy || !text.trim() || !thread || undefined} data-send>Send</button>
  </div>
</form>

<style>
  .cp { display: grid; gap: var(--s-2); }
  .opts { display: flex; flex-wrap: wrap; gap: var(--s-3); } .o { display: grid; gap: 2px; } .l { font-size: var(--text-label); color: var(--ink-3); text-transform: capitalize; }
  select { min-height: 36px; border-radius: var(--r-select); border: 0; padding: 0 var(--s-3); background: color-mix(in oklab, var(--ink) 8%, transparent); color: var(--ink); }
  .row { display: flex; gap: var(--s-3); align-items: flex-end; }
  textarea { flex: 1; min-height: 44px; padding: var(--s-3) var(--s-4); border: 0; border-radius: var(--r-select); background: color-mix(in oklab, var(--ink) 8%, transparent); color: var(--ink); font-size: 16px; resize: vertical; }
  .send { min-height: 44px; padding: 0 var(--s-5); border-radius: var(--r-pill); background: var(--accent); color: var(--on-accent, white); font-weight: var(--strong-weight); } .send[aria-disabled] { opacity: .5; }
  .err { padding: var(--s-3) var(--s-4); border-radius: var(--r-select); background: color-mix(in oklab, var(--danger) 12%, transparent); color: var(--danger); }
</style>
