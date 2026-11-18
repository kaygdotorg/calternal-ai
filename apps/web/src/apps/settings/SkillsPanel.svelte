<script lang="ts">
  // The space's skills: a list (name, one line, which agents use it, the file beside it) and a hand-held way to write a new one, following the Agent Skills standard (https://agentskills.io/specification).
  // The last step creates `AI/Skills/<name>/SKILL.md` and opens it in the Notes editor.
  import { Button, toast } from "@calternal/ui";
  import { navigate } from "../../lib/route.svelte";
  import { spaces } from "../../shell/spaces.svelte";
  import { AGENTS, descriptionHint, descriptionProblem, DESCRIPTION_MAX, nameProblem, NAME_MAX, renderSkill, skillPath, slugify, STARTER } from "./skills/skill";
  import { NEEDS, skillsStore } from "./skills/store.svelte";
  import Wizard from "./skills/Wizard.svelte";
  import "./panel.css";
  const space = $derived(spaces.current.id);
  const list = $derived(skillsStore.skills(space));
  let adding = $state(false), index = $state(0), busy = $state(false), name = $state(""), title = $state(""), description = $state(""), body = $state(STARTER), agents = $state<string[]>([]);
  const STEPS = ["Name", "Description", "Instructions", "Who uses it", "Review"];
  function begin() { adding = true; index = 0; name = ""; title = ""; description = ""; body = STARTER; agents = []; }
  const valid = (i: number): string | null => i === 0 ? nameProblem(name) : i === 1 ? descriptionProblem(description) : null;
  const preview = $derived(renderSkill({ name, description, agents, body }));
  async function create() {
    busy = true;
    try { const id = await skillsStore.addSkill(space, { name, description, agents, body }); adding = false; toast(`Created ${skillPath(name)}`); navigate(`/notes/${id}`); }
    catch (e) { toast(`Could not create the skill: ${(e as Error).message}`); } finally { busy = false; }
  }
  const toggle = (a: string) => (agents = agents.includes(a) ? agents.filter((x) => x !== a) : [...agents, a]);
</script>

<div class="sp">
  {#if skillsStore.unavailable}
    <div class="r"><span class="n"><span class="t">Skills are not on this server yet</span><span class="h">Reading and writing skills needs the server to offer: {NEEDS.filter((n) => /^(skills|agents|notes)/.test(n)).map((n) => n.split(" ")[0]).join(", ")}.</span></span></div>
  {:else if adding}
    <Wizard steps={STEPS} bind:index {valid} {busy} finish="Create and open" onfinish={create} oncancel={() => (adding = false)}>
      {#snippet children(i)}
        {#if i === 0}
          <p class="h">A skill is a folder with a file called SKILL.md. Its name is the folder's name: lowercase letters, digits and single hyphens, up to {NAME_MAX} characters.</p>
          <input class="field" aria-label="What is the skill for? (a few words)" placeholder="What is it for? For example: Review pull requests" bind:value={title} oninput={() => (name = slugify(title))} />
          <input class="field" aria-label="Skill name" placeholder="review-pull-requests" bind:value={name} spellcheck="false" autocapitalize="off" aria-invalid={!!name && !!nameProblem(name)} />
          <p class="h" class:e={!!name && !!nameProblem(name)}>{name ? nameProblem(name) ?? `Folder: ${skillPath(name).replace("/SKILL.md", "/")}` : "The name appears here as you type."}</p>
        {:else if i === 1}
          <p class="h">Say what it does and when to use it: agents read this line to decide whether the skill applies. Mention the words a person would use to ask for it.</p>
          <textarea class="field" rows="4" aria-label="Description" placeholder="Reviews a diff for correctness and style. Use when asked to review a pull request." bind:value={description}></textarea>
          <p class="h" class:e={description.length > DESCRIPTION_MAX}>{description.length} of {DESCRIPTION_MAX}{descriptionHint(description) ? ` · ${descriptionHint(description)}` : ""}</p>
        {:else if i === 2}
          <p class="h">The instructions an agent follows once it picks the skill: steps, examples, edge cases. Markdown. You can keep editing it in Notes afterwards.</p>
          <textarea class="field mono" rows="9" aria-label="Instructions" bind:value={body}></textarea>
        {:else if i === 3}
          <p class="h">Which agents may use it. None chosen means every agent in this space.</p>
          <div class="a">{#each AGENTS as a (a)}<Button variant={agents.includes(a) ? "primary" : "quiet"} text={a} aria-pressed={agents.includes(a)} onclick={() => toggle(a)} />{/each}</div>
        {:else}
          <p class="h">This is the file that will be written to <code>{skillPath(name)}</code>.</p>
          <pre class="cmd">{preview}</pre>
        {/if}
      {/snippet}
    </Wizard>
  {:else}
    {#each list as s (s.name)}
      <div class="r">
        <span class="n"><span class="t">{s.name}</span><span class="h">{s.description}</span><span class="h">Used by {s.agents.length ? s.agents.join(", ") : "every agent in this space"} · <code>{s.path}</code></span></span>
        <span class="a"><Button variant="quiet" text="Open file" onclick={() => navigate("/notes")} /></span>
      </div>
    {/each}
    {#if !list.length}<div class="r"><span class="n"><span class="t">No skills yet</span><span class="h">A skill teaches agents a procedure they can follow when it fits.</span></span></div>{/if}
    <div class="r"><span class="n"><span class="h">Skills follow the open <a href="https://agentskills.io/specification" target="_blank" rel="noreferrer">Agent Skills</a> format.</span></span><span class="a"><Button variant="primary" text="New skill" onclick={begin} /></span></div>
  {/if}
</div>
