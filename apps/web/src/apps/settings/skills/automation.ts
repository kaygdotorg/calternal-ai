/**
 * Automations: a prompt run on a schedule, each run its own thread. An automation is a Markdown file, `AI/Automations/<name>.md`: frontmatter (`id`, `name`, `schedule` as a systemd `OnCalendar`
 * expression, `timezone`, `enabled`) and the prompt as the body. Pure: the text of the file and the list's wording; the server keeps the file, runs it and records each run.
 */
import { slugify } from "./skill";

/** An automation as the list shows it. */
export interface Automation { id: string; name: string; /** The schedule in words ("every weekday at 9:00"). */ human: string; onCalendar: string; timezone: string; enabled: boolean; prompt: string; /** When it runs next, in words; empty when paused. */ next: string; /** How the last run ended; null before the first run. */ last: { ok: boolean; when: string; thread: string } | null; path: string }

/** The file of an automation in the space. */
export const automationPath = (name: string): string => `AI/Automations/${slugify(name) || "automation"}.md`;

const quote = (s: string) => JSON.stringify(s);

/** The text of the automation's file: frontmatter, then the prompt. */
export function renderAutomation(a: { id: string; name: string; onCalendar: string; timezone: string; enabled: boolean; prompt: string }): string {
  return `---\nid: ${a.id}\nname: ${quote(a.name.trim())}\nschedule: ${quote(a.onCalendar)}\ntimezone: ${a.timezone}\nenabled: ${a.enabled}\n---\n\n${a.prompt.trim()}\n`;
}
