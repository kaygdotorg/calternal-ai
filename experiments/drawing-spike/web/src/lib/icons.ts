// The host's icon vocabulary: the names a declaration may use, mapped to the web icon set (packages/ui) here and to SF Symbols
// in the Apple renderer. A name outside it draws the generic file glyph. Declarations used "book" and "chat", which
// packages/ui does not have; this table is where a closed icon vocabulary has to live (report, change V7).
import type { IconName } from "@calternal/ui";
const MAP: Record<string, IconName> = { book: "notes", chat: "threads", edit: "edit", check: "check", spark: "spark", bell: "bell", notes: "notes", plus: "plus", claude: "claude", codex: "codex" };
export const glyph = (name: string | undefined): IconName => MAP[name ?? ""] ?? "file";
