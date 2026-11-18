/**
 * Skills and automations on the real server (the operations of issue #57, `crates/core`: `skills.*`, `automations.*`). Typed calls over `server.operate`; the screens (apps/settings/SkillsPanel.svelte, AutomationsPanel.svelte)
 * keep their fixtures when the server is not selected. The schedule is typed in words, the server answers with the normalised expression, the plain wording, the creator's zone and the next instant to show back before saving.
 */
import { server } from "./client";

/** A skill in the list: its name, one line, the agents that use it (empty: all) and its file. */
export interface SkillRow { id: string | null; name: string; description: string; agents: string[]; path?: string }
/** One run's outcome: typed status, when it was scheduled and when it happened, why, and the thread it made. */
export interface RunOutcome { status: string; at_ms: number; scheduled_at_ms: number | null; reason: string | null; thread_id: string | null }
/** An automation as the server lists it. */
export interface AutomationRow { id: string; name: string; enabled: boolean; human: string; schedule: string; schedule_text: string; timezone: string; account_timezone: string | null; next_run_ms: number | null; last?: RunOutcome | null; machine_id: string | null; mode: string | null; path: string; prompt: string | null; revision: string; stop_reason: string | null }
/** A reviewed schedule: what to show back, and what to save. */
export interface SchedulePreview { schedule: string; schedule_text: string; human: string; timezone: string; next_run_ms: number | null }

/** The space's skills. */
export const listSkills = async (space_id: string): Promise<SkillRow[]> => (await server.operate<{ skills: SkillRow[] }>("skills.list", { space_id })).skills;
/** One skill's file and revision. */
export const readSkill = (space_id: string, name: string): Promise<{ skill: { markdown?: string; revision?: string } }> => server.operate("skills.read", { space_id, name });
/** Creates a skill from its SKILL.md text. */
export const createSkill = (space_id: string, name: string, markdown: string): Promise<unknown> => server.operate("skills.create", { space_id, name, markdown, base_revision: null });
/** Moves a skill's bundle to Trash (the current revision is read first: the server refuses a stale one). */
export async function removeSkill(space_id: string, name: string): Promise<void> {
  const r = await readSkill(space_id, name);
  await server.operate("skills.remove", { space_id, name, base_revision: r.skill.revision ?? "" });
}

/** The space's automations with their next and last runs. */
export const listAutomations = async (space_id: string): Promise<AutomationRow[]> => (await server.operate<{ automations: AutomationRow[] }>("automations.list", { space_id })).automations;
/** Parses words like "every weekday at 9:00" and previews them in the creator's zone (a refusal carries a supported example). */
export const previewSchedule = (space_id: string, schedule_text: string): Promise<SchedulePreview> => server.operate("automations.preview_schedule", { space_id, schedule_text });
/** Creates an automation from a reviewed schedule. */
export const createAutomation = (a: { space_id: string; name: string; prompt: string; preview: SchedulePreview }): Promise<unknown> =>
  server.operate("automations.create", { space_id: a.space_id, name: a.name, prompt: a.prompt, schedule: a.preview.schedule, schedule_text: a.preview.schedule_text, timezone: a.preview.timezone, machine_id: null, mode: null });
/** Pauses or resumes one (`base_revision` is the list's revision of it). */
export const setAutomationEnabled = (space_id: string, a: { id: string; revision: string }, on: boolean): Promise<unknown> => server.operate(on ? "automations.enable" : "automations.pause", { space_id, id: a.id, base_revision: a.revision });
/** Removes one. */
export const removeAutomation = (space_id: string, a: { id: string; revision: string }): Promise<unknown> => server.operate("automations.remove", { space_id, id: a.id, base_revision: a.revision });
/** Runs one now (works while paused); returns the thread it started. */
export const runAutomationNow = async (space_id: string, id: string): Promise<string> => (await server.operate<{ thread_id: string }>("automations.run_now", { space_id, id })).thread_id;
