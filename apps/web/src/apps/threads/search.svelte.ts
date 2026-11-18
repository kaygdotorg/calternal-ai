/** Whether the message search (ThreadSearch.svelte) is open, and the words it starts with (the palette's "Search messages for ..." row fills them). One object, set by the keyboard shortcut Mod+Shift+F, the palette and anything else that starts a thread search. */
export const messageSearch = $state({ open: false, text: "" });
