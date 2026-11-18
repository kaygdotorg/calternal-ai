/**
 * The thread search's one owner of its state (ThreadSearch.svelte draws it): whether it is open and the words it starts with. Everything that starts a search calls `startMessageSearch`: the keyboard shortcut (App.svelte, Mod+Shift+F) and the palette's
 * "Search messages for ..." row (shell/Palette.svelte); nothing else writes these fields except the panel closing and taking the words.
 */
import { client } from "./store.svelte";

/** The panel's state. Read it, never set it from outside this file (use the functions). */
export const messageSearch = $state({ open: false, text: "" });

/** Whether the server can search messages at all (the panel says so when it cannot). */
export const messageSearchAvailable = (): boolean => typeof client.searchMessages === "function";

/** Opens the thread search, optionally with words already typed (from the palette). */
export function startMessageSearch(text = ""): void { messageSearch.text = text; messageSearch.open = true; }
/** Hands the panel the words it was started with, once. */
export function takeMessageSearchText(): string { const t = messageSearch.text; messageSearch.text = ""; return t; }
/** Closes the panel. */
export function closeMessageSearch(): void { messageSearch.open = false; }
