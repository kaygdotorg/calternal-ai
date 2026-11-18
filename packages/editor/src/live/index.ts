/** The live-room pieces the app loads when a note opens (lazily; nothing here is in the first load). */
export { liveExtension, SHOW_LABELS, type Person } from './extension'
export { NoteSession, pendingNotes, stopNotes } from '../note'
export { LiveSocket } from '../sync/live'
export { StatusBox, describe as describeStatus } from '../sync/status'
