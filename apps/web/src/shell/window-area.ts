/**
 * The one boundary between the shell and whatever fills the space between its sidebars. The shell asks for the active window area and draws it; an area (today the canvas of windows, canvas/area.ts) implements the interface and
 * registers itself once at start-up. With no active area the shell shows the app in a single card. Nothing else in the shell knows about the canvas, so it can become a shell plugin when the shell has plugin slots (no
 * plugin system is built for it now).
 */
import type { Component } from "svelte";

/** A window area: what fills the space between the sidebars. */
export interface WindowArea {
  /** A stable name. */
  id: string;
  /** Whether this area is in use now (a person's setting, possibly overridden for a visit). Reactive: read inside a derived or an effect. */
  active(): boolean;
  /** The component that fills the area, fetched on first use. It reads the shell's state (the current app and address) itself; the shell passes nothing. */
  load(): Promise<Component>;
}

const areas: WindowArea[] = [];

/** Registers a window area (once, at start-up). A later one with the same id replaces the earlier. */
export function registerWindowArea(a: WindowArea): void {
  const i = areas.findIndex((x) => x.id === a.id);
  if (i >= 0) areas[i] = a; else areas.push(a);
}

/** The area in use now, or null when the shell shows the single card. */
export function activeWindowArea(): WindowArea | null {
  return areas.find((a) => a.active()) ?? null;
}
