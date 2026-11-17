/**
 * Fixture storage for the settings that have no real backend yet: values keyed by setting id (`<plugin>.<setting>`), kept in localStorage. A setting that
 * belongs to something that exists (appearance, streaming) reads and writes that instead (registry.ts). The server will keep these per account or per space.
 */
const KEY = "cal.settings";
function read(): Record<string, unknown> {
  try { return JSON.parse(localStorage.getItem(KEY) ?? "{}") as Record<string, unknown>; } catch { return {}; }
}
class Values {
  private v = $state<Record<string, unknown>>(read());
  get<T>(id: string, fallback: T): T { return (id in this.v ? this.v[id] : fallback) as T; }
  set(id: string, value: unknown): void {
    this.v[id] = value;
    try { localStorage.setItem(KEY, JSON.stringify(this.v)); } catch { /* private mode */ }
  }
}
/** The fixture values. */
export const values = new Values();
