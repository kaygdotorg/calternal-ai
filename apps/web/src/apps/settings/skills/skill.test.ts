import { describe, expect, it } from "vitest";
import { automationPath, renderAutomation } from "./automation";
import { descriptionHint, descriptionProblem, nameProblem, renderSkill, slugify } from "./skill";

describe("skill names (Agent Skills standard)", () => {
  it("accepts lowercase, digits and single hyphens", () => { expect(nameProblem("review-pull-requests")).toBeNull(); expect(nameProblem("a1")).toBeNull(); });
  it("refuses what the standard refuses", () => {
    for (const bad of ["", "Review", "has space", "-a", "a-", "a--b", "a".repeat(65)]) expect(nameProblem(bad), bad).not.toBeNull();
  });
  it("makes a valid name from words", () => { expect(slugify("Review Pull Requests!")).toBe("review-pull-requests"); expect(nameProblem(slugify("  Écrire — un test "))).toBeNull(); expect(slugify("A".repeat(70)).length).toBe(64); });
});
describe("descriptions", () => {
  it("needs words, at most 1024 characters, and nudges for a when", () => {
    expect(descriptionProblem("")).not.toBeNull(); expect(descriptionProblem("x".repeat(1025))).not.toBeNull(); expect(descriptionProblem("Reviews diffs. Use when asked to review.")).toBeNull();
    expect(descriptionHint("Reviews diffs")).not.toBeNull(); expect(descriptionHint("Reviews diffs. Use when asked.")).toBeNull();
  });
});
describe("files", () => {
  it("SKILL.md has frontmatter with name, description and agents, then the body", () => {
    const t = renderSkill({ name: "a-b", description: "Does x.\nUse when y.", agents: ["Claude"], body: "## Steps" });
    expect(t).toBe('---\nname: a-b\ndescription: "Does x. Use when y."\nmetadata:\n  agents: "Claude"\n---\n\n## Steps\n');
  });
  it("an automation file keeps the schedule as OnCalendar with its time zone", () => {
    expect(automationPath("Morning Digest")).toBe("AI/Automations/morning-digest.md");
    expect(renderAutomation({ id: "x", name: "Morning digest", onCalendar: "Mon..Fri *-*-* 09:00:00", timezone: "Europe/Paris", enabled: true, prompt: "Summarize." }))
      .toBe('---\nid: x\nname: "Morning digest"\nschedule: "Mon..Fri *-*-* 09:00:00"\ntimezone: Europe/Paris\nenabled: true\n---\n\nSummarize.\n');
  });
});
