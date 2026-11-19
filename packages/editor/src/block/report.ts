/** Where the host hears about the note's frontmatter. A facet in its own small module so the editor can offer it without loading the lazy frontmatter piece (frontmatter.ts), which reads it. */
import { Facet } from "@codemirror/state";

/** A host callback given the note's `key: value` frontmatter pairs; see `frontmatterWatch` in frontmatter.ts for when it is called. */
export const frontmatterReport = Facet.define<(fm: Record<string, string>) => void, ((fm: Record<string, string>) => void) | null>({ combine: (v) => v[0] ?? null });
