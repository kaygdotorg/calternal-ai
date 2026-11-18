/**
 * The account rules the onboarding checks before it asks the server (docs/account-onboarding.md on lane/accounts, "UI validation"); the server is the authority and repeats every check. Pure.
 */

/** Names the server keeps for itself (case-insensitive); UUID-shaped names are reserved too. */
const RESERVED = new Set(["admin", "administrator", "root", "owner", "system", "support", "calternal"]);
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** What is wrong with a username, in plain words, or null when it can be sent. 3 to 32 ASCII letters, digits or single interior hyphens; the server trims and lowercases. */
export function usernameProblem(raw: string): string | null {
  const u = raw.trim();
  if (!u) return "Choose a username.";
  if (u.length < 3) return "A username has at least 3 characters.";
  if (u.length > 32) return "A username has at most 32 characters.";
  if (!/^[A-Za-z0-9-]+$/.test(u)) return "Use only letters, digits and hyphens.";
  if (u.startsWith("-") || u.endsWith("-")) return "A hyphen cannot start or end a username.";
  if (u.includes("--")) return "Use single hyphens only.";
  if (RESERVED.has(u.toLowerCase()) || UUID.test(u)) return "That name is kept by the server. Choose another.";
  return null;
}

/** Whether an address looks like one (one `@`, something on each side, a dot after it): a typo catcher, not validation; the mail itself is the proof. */
export const emailLooksOk = (raw: string): boolean => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(raw.trim());

/** The email code is eight decimal digits (leading zeroes kept, so it is text). */
export const emailCodeOk = (raw: string): boolean => /^[0-9]{8}$/.test(raw.trim());

/** A recovery code is 32 hexadecimal characters; shown in four groups of eight so it can be read and copied by eye. */
export const groupCode = (code: string): string => (code.match(/.{1,8}/g) ?? [code]).join(" ");

/** The set as one text block, one code per line, ungrouped exactly as the server issued them (copying and the downloaded file use this). */
export const codesText = (codes: string[]): string => codes.join("\n") + "\n";

/** What the person types back may have spaces or hyphens put in for reading; the server gets only the code. */
export const cleanCode = (raw: string): string => raw.replace(/[\s-]/g, "").toLowerCase();
