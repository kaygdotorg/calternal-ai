/** Mockup-only state of the note page: whether the text is at the interface's body size ("standard") or one step larger with a longer line height ("reading"). Set with the "Aa" button on a note, the Text control in the panel, or `?reading=1` in the address; remembered in the browser. */
const KEY = "mockup-notes-reading", forced = new URLSearchParams(location.search).get("reading");
class NotesUi {
  reading = $state(forced ? forced === "1" : localStorage.getItem(KEY) === "1");
  set(v: boolean) { this.reading = v; localStorage.setItem(KEY, v ? "1" : "0"); }
}
export const notesUi = new NotesUi();
