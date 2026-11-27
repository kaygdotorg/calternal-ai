/** What each Calendar window is showing (the view, the day it is on, the selected day, the calendars switched off). Kept per window so a person can open an event and come back to the same week; the right panel's mini month and calendar switches read and write the same record. On a phone width a window starts on Day (a month is too small to read there; Month is one tap away). The address can force a start: `?cal=week&day=2026-11-23`. */
import { TODAY } from "./dates";
export type Mode = "month" | "week" | "day" | "agenda";
export interface CalState { mode: Mode; anchor: string; sel: string; hidden: string[] }
const states: Record<string, CalState> = $state({});
const q = new URLSearchParams(location.search);
/** The state of a window, made on first use (call it from a component's script, not from a derived). */
export function calState(id: string): CalState {
  if (!states[id]) { const day = q.get("day") ?? TODAY; states[id] = { mode: (q.get("cal") as Mode) ?? (innerWidth < 720 ? "day" : "month"), anchor: day, sel: day, hidden: (q.get("off") ?? "").split(",").filter(Boolean) }; }
  return states[id]!;
}
