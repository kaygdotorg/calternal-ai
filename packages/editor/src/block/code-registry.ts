/** The doorway between the markdown language (always loaded) and the grammar list (lazy): `codeLanguage` answers null until `code.ts` registers the list. Tiny on purpose. */
import type { LanguageDescription } from "@codemirror/language";

let find: (info: string) => LanguageDescription | null = () => null;
/** Called by `code.ts` when it loads. */
export const setCodeLanguages = (f: (info: string) => LanguageDescription | null): void => { find = f; };
/** The grammar for a fence's info string, or null. */
export const codeLanguage = (info: string): LanguageDescription | null => find(info);
