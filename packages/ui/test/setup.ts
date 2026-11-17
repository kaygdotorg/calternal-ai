// jsdom has no ResizeObserver, matchMedia or layout; the surfaces only need them to exist.
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/svelte";
globalThis.ResizeObserver ??= class { observe() {} unobserve() {} disconnect() {} } as unknown as typeof ResizeObserver;
globalThis.matchMedia ??= ((q: string) => ({ matches: false, media: q, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {}, onchange: null, dispatchEvent: () => false })) as unknown as typeof matchMedia;
globalThis.requestAnimationFrame ??= ((cb: FrameRequestCallback) => setTimeout(() => cb(0), 0)) as unknown as typeof requestAnimationFrame;
afterEach(() => cleanup());

// jsdom has no Web Animations: the tooltip and the popover morph call `animate`; a finished no-op stands in.
if (!Element.prototype.animate) Element.prototype.animate = function () { const a = { cancel() {}, finish() {}, onfinish: null } as unknown as Animation; queueMicrotask(() => (a.onfinish as (() => void) | null)?.()); return a; };
