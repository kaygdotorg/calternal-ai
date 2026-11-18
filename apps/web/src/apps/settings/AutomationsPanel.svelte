<script lang="ts">
  // The space's automations: a list (schedule in words, next run, last result, Run now, pause) and a hand-held way to make one. The schedule is typed in plain words and shown back before saving ("every weekday at 9:00");
  // it is stored as a systemd OnCalendar expression with a time zone in `AI/Automations/<name>.md`. Each run is a thread. On the real server every part is an operation (server/automations.ts: the server parses the words, returns the
  // reviewed expression, wording, zone and next instant, and runs the schedule as its creator); on fixtures the same screens run on the in-memory store.
  import { Button, toast } from "@calternal/ui";
  import { navigate } from "../../lib/route.svelte";
  import { createAutomation, listAutomations, previewSchedule, removeAutomation, runAutomationNow, setAutomationEnabled, type AutomationRow, type SchedulePreview } from "../../server/automations";
  import { serverMode } from "../../server/client";
  import { say } from "../../server/say";
  import { spaces } from "../../shell/spaces.svelte";
  import { automationPath, renderAutomation } from "./skills/automation";
  import { nextWords, parseSchedule } from "./skills/schedule";
  import { skillsStore } from "./skills/store.svelte";
  import Wizard from "./skills/Wizard.svelte";
  import "./panel.css";
  const real = serverMode();
  const space = $derived(spaces.current.id);
  const list = $derived(skillsStore.automations(space));
  const tz = Intl.DateTimeFormat().resolvedOptions().timeZone;
  let adding = $state(false), index = $state(0), busy = $state(false), name = $state(""), when = $state(""), prompt = $state("");
  const STEPS = ["Name", "When", "Prompt", "Review"];
  const sched = $derived(parseSchedule(when));
  // The real server's list and the schedule review it gives back.
  let remote = $state<AutomationRow[]>([]), loaded = $state(false), error = $state("");
  let preview = $state<SchedulePreview | null>(null), previewError = $state("");
  async function load() { try { remote = await listAutomations(space); error = ""; } catch (e) { error = say(e, "load the automations"); } loaded = true; }
  $effect(() => { if (real) { void space; void load(); } });
  // The words are sent to the server a moment after typing stops; the answer (or the refusal with a supported example) is what the person reviews.
  $effect(() => {
    if (!real) return;
    const text = when.trim(), id = space;
    if (!text) { preview = null; previewError = ""; return; }
    const h = setTimeout(() => void previewSchedule(id, text).then((p) => { if (when.trim() === text) { preview = p; previewError = ""; } }, (e) => { if (when.trim() === text) { preview = null; previewError = say(e, "read that schedule"); } }), 350);
    return () => clearTimeout(h);
  });
  const okWhen = $derived(real ? !!preview : sched.ok);
  const whyNot = $derived(real ? previewError || "Say when it should run." : !sched.ok ? sched.why : "");
  function begin() { adding = true; index = 0; name = ""; when = ""; prompt = ""; preview = null; previewError = ""; }
  const valid = (i: number): string | null => i === 0 ? (name.trim() ? null : "Give the automation a name.") : i === 1 ? (okWhen ? null : whyNot) : i === 2 ? (prompt.trim() ? null : "Say what the agent should do each time.") : null;
  const at = (ms: number | null) => (ms ? new Date(ms).toLocaleString("en", { weekday: "short", day: "numeric", month: "short", hour: "numeric", minute: "2-digit" }) : "none");
  async function create() {
    busy = true;
    try {
      if (real) { if (!preview) return; await createAutomation({ space_id: space, name: name.trim(), prompt, preview }); adding = false; toast(`Created ${automationPath(name)}`); await load(); return; }
      if (!sched.ok) return;
      const id = await skillsStore.addAutomation(space, { name: name.trim(), human: sched.human, onCalendar: sched.onCalendar, timezone: tz, prompt, next: nextWords(sched.schedule, new Date()) });
      adding = false; toast(`Created ${automationPath(name)}`); navigate(`/notes/${id}`);
    } catch (e) { toast(`Could not create the automation: ${say(e)}`); } finally { busy = false; }
  }
  async function run(id: string) { try { navigate(`/threads/${real ? await runAutomationNow(space, id) : await skillsStore.runNow(space, id)}`); } catch (e) { toast(`Could not run it: ${say(e)}`); } }
  async function act(f: () => Promise<unknown>) { busy = true; try { await f(); await load(); } catch (e) { error = say(e); } busy = false; }
</script>

<div class="sp">
  {#if adding}
    <Wizard steps={STEPS} bind:index {valid} {busy} finish={real ? "Create" : "Create and open"} onfinish={create} oncancel={() => (adding = false)}>
      {#snippet children(i)}
        {#if i === 0}
          <p class="h">A short name for the list. It becomes the file name.</p>
          <input class="field" aria-label="Automation name" placeholder="Morning digest" bind:value={name} />
        {:else if i === 1}
          <p class="h">When should it run? Say it the way you would to a person: every weekday at 9:00, every Monday at 8:30, daily at 7:15, monthly at 9.</p>
          <input class="field" aria-label="When" placeholder="every weekday at 9:00" bind:value={when} />
          {#if real}
            <p class="h" class:e={!!previewError} aria-live="polite">{#if !when.trim()}Time zone: your account's (the server uses it).{:else if preview}Runs {preview.human} ({preview.timezone}). Next: {at(preview.next_run_ms)}.{:else if previewError}{previewError}{:else}Reading it…{/if}</p>
          {:else}
            <p class="h" class:e={!!when && !sched.ok} aria-live="polite">{#if !when}Time zone: {tz}.{:else if sched.ok}Runs {sched.human} ({tz}). Next: {nextWords(sched.schedule, new Date())}.{:else}{sched.why}{/if}</p>
          {/if}
        {:else if i === 2}
          <p class="h">What should the agent do each time? Every run starts its own thread with this as the first message.</p>
          <textarea class="field" rows="5" aria-label="Prompt" placeholder="Summarize what changed overnight and list anything that needs me." bind:value={prompt}></textarea>
        {:else if real && preview}
          <p class="h">Runs {preview.human} in {preview.timezone}. Next: {at(preview.next_run_ms)}. The schedule saved is this expression:</p>
          <pre class="cmd">{preview.schedule}</pre>
          <p class="h">File: <code>{automationPath(name)}</code>. It runs as you, on your machine and provider.</p>
        {:else if sched.ok}
          <p class="h">Runs {sched.human}. File: <code>{automationPath(name)}</code></p>
          <pre class="cmd">{renderAutomation({ id: "(made on create)", name, onCalendar: sched.onCalendar, timezone: tz, enabled: true, prompt })}</pre>
        {/if}
      {/snippet}
    </Wizard>
  {:else if real}
    {#each remote as a (a.id)}
      <div class="r col">
        <span class="n"><span class="t">{a.name}</span><span class="h">Runs {a.human} · {a.enabled ? `next ${at(a.next_run_ms)}` : a.stop_reason ? `stopped: ${a.stop_reason}` : "paused"}</span><span class="h">{a.last ? `Last run ${at(a.last.at_ms)}: ${a.last.status}${a.last.reason ? ` (${a.last.reason})` : ""}` : "Not run yet"} · <code>{a.path}</code></span></span>
        <span class="a"><Button variant="secondary" text="Run now" disabled={busy} onclick={() => run(a.id)} /><Button variant="secondary" text={a.enabled ? "Pause" : "Resume"} disabled={busy} onclick={() => act(() => setAutomationEnabled(space, a, !a.enabled))} /><Button variant="secondary" text="Remove" disabled={busy} onclick={() => confirm(`Remove ${a.name}?`) && act(() => removeAutomation(space, a))} /></span>
      </div>
    {/each}
    {#if loaded && !remote.length && !error}<div class="r"><span class="n"><span class="t">No automations yet</span><span class="h">An automation starts a thread on a schedule.</span></span></div>{/if}
    {#if error}<p class="e" role="alert">{error}</p>{/if}
    <div class="r"><span class="n"><span class="h">Schedules are stored as systemd timer expressions, with a time zone. A run belongs to the person who made the automation.</span></span><span class="a"><Button class="settings-action" variant="primary" text="New automation" onclick={begin} /></span></div>
  {:else}
    {#each list as a (a.id)}
      <div class="r col">
        <span class="n"><span class="t">{a.name}</span><span class="h">Runs {a.human} · {a.enabled ? `next ${a.next}` : "paused"}</span><span class="h">{a.last ? `Last run ${a.last.when}: ${a.last.ok ? "done" : "failed"}` : "Not run yet"} · <code>{a.path}</code></span></span>
        <span class="a"><Button variant="secondary" text="Run now" onclick={() => run(a.id)} /><Button variant="secondary" text={a.enabled ? "Pause" : "Resume"} onclick={() => skillsStore.setEnabled(space, a.id, !a.enabled)} /><Button variant="secondary" text="Open file" onclick={() => navigate("/notes")} /></span>
      </div>
    {/each}
    {#if !list.length}<div class="r"><span class="n"><span class="t">No automations yet</span><span class="h">An automation starts a thread on a schedule.</span></span></div>{/if}
    <div class="r"><span class="n"><span class="h">Schedules are stored as systemd timer expressions, with a time zone.</span></span><span class="a"><Button class="settings-action" variant="primary" text="New automation" onclick={begin} /></span></div>
  {/if}
</div>
