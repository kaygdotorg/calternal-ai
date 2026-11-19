/**
 * Whether this renderer can draw a description at all (docs/description-contract.md section 7): the version, the kind and the features it `requires`. A description it cannot draw is refused whole,
 * with the reason and what this renderer supports, rather than drawn wrongly. Unknown fields are ignored; unknown enum values the renderer needs are refused where they are read.
 */
import type { AppDescription } from "./model";

/** The description versions and `requires` features this renderer draws. */
export const SUPPORTED = { versions: [2], features: ["bind-search"] } as const;

/** The refusal a page shows or relays (`{native: false, reason, supported}`). */
export interface Refusal { native: false; reason: string; supported: readonly number[] }

/** Returns a refusal when the description cannot be drawn, else null. */
export function refusalOf(d: unknown): Refusal | null {
  const a = d as Partial<AppDescription> | null;
  const supported = SUPPORTED.versions;
  if (!a || typeof a !== "object") return { native: false, reason: "not a description", supported };
  if (!(SUPPORTED.versions as readonly number[]).includes(a.version as number)) return { native: false, reason: `version ${String(a.version)} is not drawn here`, supported };
  if (a.kind !== "app") return { native: false, reason: `kind ${String(a.kind)} is not an app description`, supported };
  const unknown = (a.requires ?? []).filter((f) => !(SUPPORTED.features as readonly string[]).includes(f));
  if (unknown.length) return { native: false, reason: `requires ${unknown.join(", ")}`, supported };
  return null;
}
