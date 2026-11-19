/**
 * Where a change came from. Automatic transforms (a link, a list continuation, a picked mention) run only for the person's own typing; a change that arrives from elsewhere,
 * is replayed from unsent input or is made by a command never triggers them again (docs/editor-port-plan.md; ADR 0020, "Editor").
 */
export type Origin = "user" | "remote" | "replay" | "command";

import type { Transaction } from "@codemirror/state";
import { ySyncAnnotation } from "y-codemirror.next";

/** True when a transaction came from the shared document, not from this editor (the existing editor's test, from y-codemirror.next). */
export const isRemote = (tr: Transaction): boolean => tr.annotation(ySyncAnnotation) != null;
/** True only for characters the person typed into this editor just now. */
export const isOwnTyping = (tr: Transaction): boolean => tr.docChanged && !isRemote(tr) && tr.isUserEvent("input.type");

/** True for the person's own keystrokes and pastes. */
export const isOwn = (o: Origin): boolean => o === "user";
/** True for changes that did not originate in this editor's person (they wait for an input method, and are never undone by this person's undo). */
export const isForeign = (o: Origin): boolean => o === "remote" || o === "replay";
