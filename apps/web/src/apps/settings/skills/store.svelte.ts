/**
 * What the Skills and Automations pages list, per space. On fixtures it is kept in memory and creating one really writes the file as a note (so the Notes editor opens it); on the real server the
 * operations below do not exist yet, so the pages say so (see `NEEDS`) instead of pretending. Every list is read by the space's id.
 */
import { serverMode } from "../../../server/client";
import { startThread } from "../../threads/store.svelte";
import { automationPath, renderAutomation, type Automation } from "./automation";
import { nextWords, parseSchedule } from "./schedule";
import { renderSkill, skillPath, type Skill } from "./skill";

/** The server operations these pages need (none exists yet): the list the pages are built against. */
export const NEEDS = [
  "skills.list {space_id} exists (name, description): it also needs the agents that use each skill and the file's note id so Open file works",
  "skills.create / skills.update / skills.delete {space_id, name, ...}: write AI/Skills/<name>/SKILL.md (a folder; optional scripts/, references/, assets/)",
  "agents.list {space_id} -> [{id, label}]: the agents a skill can be given to",
  "automations.list {space_id} -> [{id, name, schedule, timezone, enabled, next_run_ms, last:{ok, at_ms, thread_id}}]: read from AI/Automations/*.md",
  "automations.create / update / delete / pause {space_id, ...}: write the file and (re)arm the systemd timer",
  "automations.run_now {space_id, id} -> {thread_id}: starts the automation's prompt as a new thread now",
  "notes.create with a path (not only a title), so a file can be written to AI/Skills/<name>/SKILL.md and opened in the Notes editor",
];

const tz = () => Intl.DateTimeFormat().resolvedOptions().timeZone;
const seed = (): { skills: Skill[]; automations: Automation[] } => {
  const sched = parseSchedule("every weekday at 9:00");
  if (!sched.ok) throw new Error("seed");
  const when = new Date();
  return {
    skills: [
      { name: "review-pull-requests", description: "Reviews a diff for correctness and style. Use when asked to review a pull request.", agents: ["Claude", "Codex"], path: skillPath("review-pull-requests") },
      { name: "release-notes", description: "Writes release notes from merged changes. Use when preparing a release.", agents: [], path: skillPath("release-notes") },
    ],
    automations: [{ id: "a-digest", name: "Morning digest", human: sched.human, onCalendar: sched.onCalendar, timezone: tz(), enabled: true, prompt: "Summarize what changed overnight.", next: nextWords(sched.schedule, when), last: { ok: true, when: "yesterday at 9:00", thread: "" }, path: automationPath("Morning digest") }],
  };
};

class SkillsStore {
  /** Each space's lists, made on first read (a plain map: a reactive one cannot be filled while a page is deriving from it); the lists inside are reactive. */
  private by = new Map<string, { skills: Skill[]; automations: Automation[] }>();
  /** True on the real server, where the automation operations in `NEEDS` are still missing (skills are listed with `skills.list`). */
  get unavailable(): boolean { return serverMode(); }
  /** The real server's skills for a space (`skills.list`: name and one line each; the file is `AI/Skills/<name>/SKILL.md` by the standard). Throws the server's error. */
  async loadSkills(space: string): Promise<Skill[]> {
    const { server } = await import("../../../server/client");
    const r = await server.operate<{ skills: { name: string; description: string }[] }>("skills.list", { space_id: space });
    return r.skills.map((k) => ({ name: k.name, description: k.description, agents: [], path: skillPath(k.name) }));
  }
  private of(space: string) { let v = this.by.get(space); if (!v) { const fresh = $state(seed()); v = fresh; this.by.set(space, v); } return v; }
  /** The space's skills. */ skills(space: string): Skill[] { return this.of(space).skills; }
  /** The space's automations. */ automations(space: string): Automation[] { return this.of(space).automations; }

  /** Writes the skill's file as a note and lists the skill; returns the note's id so the caller can open it in the editor. */
  async addSkill(space: string, s: { name: string; description: string; agents: string[]; body: string }): Promise<string> {
    const { notes } = await import("../../notes/store.svelte");
    const note = await notes.client.create(space, s.name, renderSkill(s));
    this.of(space).skills.push({ name: s.name, description: s.description, agents: s.agents, path: skillPath(s.name) });
    return note.id;
  }
  /** Writes the automation's file as a note and lists the automation; returns the note's id. */
  async addAutomation(space: string, a: { name: string; human: string; onCalendar: string; timezone: string; prompt: string; next: string }): Promise<string> {
    const { notes } = await import("../../notes/store.svelte");
    const id = crypto.randomUUID();
    const note = await notes.client.create(space, a.name, renderAutomation({ id, name: a.name, onCalendar: a.onCalendar, timezone: a.timezone, enabled: true, prompt: a.prompt }));
    this.of(space).automations.push({ id, name: a.name, human: a.human, onCalendar: a.onCalendar, timezone: a.timezone, enabled: true, prompt: a.prompt, next: a.next, last: null, path: automationPath(a.name) });
    return note.id;
  }
  /** Pauses or resumes an automation. */
  setEnabled(space: string, id: string, enabled: boolean): void {
    const a = this.of(space).automations.find((x) => x.id === id);
    if (!a) return;
    a.enabled = enabled;
    const p = parseSchedule(a.human);
    a.next = enabled && p.ok ? nextWords(p.schedule, new Date()) : "";
  }
  /** Runs it now: each run is a thread named for the automation; returns its id. */
  async runNow(space: string, id: string): Promise<string> {
    const a = this.of(space).automations.find((x) => x.id === id);
    if (!a) throw new Error("That automation is gone.");
    const t = await startThread(`${a.name}: ${new Date().toLocaleString("en", { hour: "numeric", minute: "2-digit" })}`);
    a.last = { ok: true, when: "just now", thread: t.id };
    return t.id;
  }
}
/** The shared state of both pages. */
export const skillsStore = new SkillsStore();
