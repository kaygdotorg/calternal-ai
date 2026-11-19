/**
 * Which tab of the Threads sidebar the person chose (Inbox or All Threads), kept apart from the sidebar's component so the native chrome description (shell/chrome) and the drawn sidebar read and change the same
 * choice. `null` is "not chosen yet": the sidebar then opens on the Inbox when it has something and on All Threads when it has not.
 */
export const view = $state<{ chosen: "inbox" | "all" | null }>({ chosen: null });
