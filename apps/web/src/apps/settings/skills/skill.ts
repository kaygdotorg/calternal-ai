/**
 * Skills, as the Agent Skills standard (https://agentskills.io/specification) defines them: a folder named for the skill holding `SKILL.md`, YAML frontmatter followed by Markdown instructions.
 * Required frontmatter: `name` (1 to 64 characters; lowercase letters, digits and hyphens; no leading, trailing or doubled hyphen; the same as the folder name) and `description` (1 to 1024 characters:
 * what the skill does and when to use it). Optional: `license`, `compatibility`, `metadata` (string pairs; the agents that use the skill are kept there as `agents`), `allowed-tools`. In a space the
 * folder is `AI/Skills/<name>/`. Pure: these are the rules and the text of the file; the server keeps the file.
 */

/** A skill as the list shows it. */
export interface Skill { name: string; description: string; /** The agents that use it (empty: every agent in the space). */ agents: string[]; /** The file's path in the space. */ path: string }

/** The agents a skill can be given to. */
export const AGENTS = ["Claude", "Codex"] as const;

/** The longest name and description the standard allows. */
export const NAME_MAX = 64, DESCRIPTION_MAX = 1024;

/** The folder of a skill in the space; its `SKILL.md` is inside. */
export const skillPath = (name: string): string => `AI/Skills/${name}/SKILL.md`;

/** Why this is not a valid skill name, or null when it is. */
export function nameProblem(name: string): string | null {
  if (!name) return "Give the skill a name.";
  if (name.length > NAME_MAX) return `A name has at most ${NAME_MAX} characters.`;
  if (/[A-Z]/.test(name)) return "Use lowercase letters only.";
  if (!/^[\p{Ll}\p{N}-]+$/u.test(name)) return "Use letters, digits and hyphens only (no spaces).";
  if (name.startsWith("-") || name.endsWith("-")) return "A name cannot start or end with a hyphen.";
  if (name.includes("--")) return "A name cannot have two hyphens in a row.";
  return null;
}

/** A valid name made from what was typed: "Review pull requests" becomes "review-pull-requests". */
export function slugify(text: string): string {
  return text.toLowerCase().normalize("NFKD").replace(/[̀-ͯ]/g, "").replace(/[^\p{Ll}\p{N}]+/gu, "-").replace(/^-+|-+$/g, "").slice(0, NAME_MAX).replace(/-+$/g, "");
}

/** Why this description is not usable, or null when it is. A description that says only what a skill is, and not when to use it, is allowed but gets a hint from `descriptionHint`. */
export function descriptionProblem(d: string): string | null {
  if (!d.trim()) return "Describe what the skill does and when to use it.";
  if (d.length > DESCRIPTION_MAX) return `A description has at most ${DESCRIPTION_MAX} characters.`;
  return null;
}
/** A nudge when the description will not help an agent decide to use the skill (no "when"), or null. */
export const descriptionHint = (d: string): string | null => (d.trim() && !/\b(when|if|whenever|after|before|use)\b/i.test(d) ? "Say when to use it, for example: Use when reviewing a pull request." : null);

const quote = (s: string) => JSON.stringify(s);

/** The text of `SKILL.md`: frontmatter (name, description, and the agents under `metadata`) then the instructions. */
export function renderSkill(s: { name: string; description: string; agents: string[]; body: string }): string {
  const meta = s.agents.length ? `metadata:\n  agents: ${quote(s.agents.join(", "))}\n` : "";
  return `---\nname: ${s.name}\ndescription: ${quote(s.description.trim().replace(/\s+/g, " "))}\n${meta}---\n\n${s.body.trim() || "Describe the steps here."}\n`;
}

/** The starter instructions a new skill begins with. */
export const STARTER = "## When to use\n\n- \n\n## Steps\n\n1. \n2. \n\n## Notes\n\n- ";
