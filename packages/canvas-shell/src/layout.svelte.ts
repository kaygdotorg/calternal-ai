/** The shell's widths, as docs/ui-shell.md decides them: the left sidebar 200 to 420 px, the right panel 260 to 560 px, each kept for this device and put back with Enter or a double-click on its handle. The right panel can be hidden (a button, the window menu, Alt+Shift+P). */
export const WIDTHS = { left: { min: 200, max: 420, def: 304 }, right: { min: 260, max: 560, def: 320 } } as const;
const KEY = "calternal.mockup.layout";
const q = new URLSearchParams(typeof location === "undefined" ? "" : location.search);
function load(): { left: number | null; right: number | null; open: boolean | null } {
  try { const a = JSON.parse(localStorage.getItem(KEY) ?? "null"); if (a && !q.has("fresh")) return a; } catch { /* defaults */ }
  return { left: null, right: null, open: null };
}
class Layout {
  private s = $state(load());
  /** Whether the right panel is shown: the person's choice, else on for a wide window (a phone has it as a sheet instead). */
  get rightOpen(): boolean { return this.s.open ?? (q.has("right") ? q.get("right") === "1" : typeof innerWidth !== "undefined" && innerWidth >= 1180); }
  set rightOpen(v: boolean) { this.s.open = v; this.save(); }
  /** Whether the left sidebar is shown (not saved: a reload shows it). */
  leftOpen = $state(q.get("left") !== "0");
  get left(): number { return this.s.left ?? WIDTHS.left.def; }
  get right(): number { return this.s.right ?? WIDTHS.right.def; }
  setWidth(side: "left" | "right", px: number | null) { const r = WIDTHS[side]; this.s[side] = px === null ? null : Math.max(r.min, Math.min(r.max, Math.round(px))); this.save(); }
  private save() { try { localStorage.setItem(KEY, JSON.stringify(this.s)); } catch { /* private mode */ } }
}
export const layout = new Layout();
