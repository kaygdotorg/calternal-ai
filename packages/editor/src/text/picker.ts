/**
 * The keys of an open picker, the same everywhere: Up and Down move through the choices (wrapping), Enter or Tab chooses, Escape closes. Returns true when it took the key, so the host calls
 * `preventDefault` there and the text field or editor does nothing else with it.
 */
export interface PickerState { count: number; index: number; set(i: number): void; choose(): void; dismiss(): void }

/** Handles a keydown for an open picker; false when the picker is empty or the key is not one of its keys. */
export function pickerKey(e: { key: string; preventDefault(): void }, p: PickerState): boolean {
  if (!p.count) return false;
  if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); p.set((p.index + (e.key === "ArrowDown" ? 1 : p.count - 1)) % p.count); return true; }
  if (e.key === "Enter" || e.key === "Tab") { e.preventDefault(); p.choose(); return true; }
  if (e.key === "Escape") { e.preventDefault(); p.dismiss(); return true; }
  return false;
}
