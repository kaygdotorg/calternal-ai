/**
 * Shared canvas shell package (`@calternal/canvas-shell`) imported by both
 * `apps/mockup` and `apps/web` so there is one single source of truth for the
 * snapping canvas, window chrome, sidebar chrome, bottom-centre strip, and motion.
 */
export { default as Canvas } from "./canvas/Canvas.svelte";
export { default as Sidebar } from "./shell/Sidebar.svelte";
export { default as AppStrip, type StripItem } from "./shell/AppStrip.svelte";
export { default as Av } from "./shell/Av.svelte";
export { default as AppIcon } from "./shell/AppIcon.svelte";
export { Windows, createWindows, type Win, type WinKind } from "./windows.svelte";
export { layout, WIDTHS } from "./layout.svelte";
export { bezier, reduced, ms, tween, rise } from "./motion";
export { target, openNew, watchNewWindow, type Target } from "./lib/intent";
