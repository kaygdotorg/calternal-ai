<script lang="ts">
  // The space's automations: a list (schedule in words, next run, last result, Run now, pause) and a hand-held way to make one. The schedule is typed in plain words, read with the palette's recogniser and shown back
  // ("every weekday at 9:00"); it is stored as a systemd OnCalendar expression with a time zone in `AI/Automations/<name>.md`. Each run is a thread.
  import { Button, toast } from "@calternal/ui";
  import { navigate } from "../../lib/route.svelte";
  import { spaces } from "../../shell/spaces.svelte";
  import { automationPath, renderAutomation } from "./skills/automation";
  import { nextWords, parseSchedule } from "./skills/schedule";
  import { NEEDS, skillsStore } from "./skills/store.svelte";
  import Wizard from "./skills/Wizard.svelte";
  import "./panel.css";
  const space = $derived(spaces.current.id);
  const list = $derived(skillsStore.automations(space));
  const tz = Intl.DateTimeFormat().resolvedOptions().timeZone;
  let adding = $state(false), index = $state(0), busy = $state(false), name = $state(""), when = $state(""), prompt = $state("");
  const STEPS = ["Name", "When", "Prompt", "Review"];
  const sched = $derived(parseSchedule(when));
  function begin() { adding = true; index = 0; name = ""; when = ""; prompt = ""; }
  const valid = (i: number): string | null => i === 0 ? (name.trim() ? null : "Give the automation a name.") : i === 1 ? (sched.ok ? null : sched.why) : i === 2 ? (prompt.trim() ? null : "Say what the agent should do each time.") : null;
  async function create() {
    if (!sched.ok) return;
    busy = true;
    try {
      const id = await skillsStore.addAutomation(space, { name: name.trim(), human: sched.human, onCalendar: sched.onCalendar, timezone: tz, prompt, next: nextWords(sched.schedule, new Date()) });
      adding = false; toast(`Created ${automationPath(name)}`); navigate(`/notes/${id}`);
    } catch (e) { toast(`Could not create the automation: ${(e as Error).message}`); } finally { busy = false; }
  }
  async function run(id: string) { try { navigate(`/threads/${await skillsStore.runNow(space, id)}`); } catch (e) { toast(`Could not run it: ${(e as Error).message}`); } }
</script>

<div class="sp">
  {#if skillsStore.unavailable}
    <div class="r"><span class="n"><span class="t">Automations are not on this server yet</span><span class="h">They need the server to offer: {NEEDS.filter((n) => /^automations/.test(n)).map((n) => n.split(" ")[0]).join(", ")}.</span></span></div>
  {:else if adding}
    <Wizard steps={STEPS} bind:index {valid} {busy} finish="Create and open" onfinish={create} oncancel={() => (adding = false)}>
      {#snippet children(i)}
        {#if i === 0}
          <p class="h">A short name for the list. It becomes the file name.</p>
          <input class="field" aria-label="Automation name" placeholder="Morning digest" bind:value={name} />
        {:else if i === 1}
          <p class="h">When should it run? Say it the way you would to a person: every weekday at 9:00, every Monday at 8:30, daily at 7:15, monthly at 9.</p>
          <input class="field" aria-label="When" placeholder="every weekday at 9:00" bind:value={when} />
          <p class="h" class:e={!!when && !sched.ok} aria-live="polite">{#if !when}Time zone: {tz}.{:else if sched.ok}Runs {sched.human} ({tz}). Next: {nextWords(sched.schedule, new Date())}.{:else}{sched.why}{/if}</p>
        {:else if i === 2}
          <p class="h">What should the agent do each time? Every run starts its own thread with this as the first message.</p>
          <textarea class="field" rows="5" aria-label="Prompt" placeholder="Summarize what changed overnight and list anything that needs me." bind:value={prompt}></textarea>
        {:else if sched.ok}
          <p class="h">Runs {sched.human}. File: <code>{automationPath(name)}</code></p>
          <pre class="cmd">{renderAutomation({ id: "(made on create)", name, onCalendar: sched.onCalendar, timezone: tz, enabled: true, prompt })}</pre>
        {/if}
      {/snippet}
    </Wizard>
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
