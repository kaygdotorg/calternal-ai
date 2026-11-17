// jsdom has no ResizeObserver, matchMedia or layout; the surfaces only need them to exist.
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/svelte";
globalThis.ResizeObserver ??= class { observe() {} unobserve() {} disconnect() {} } as unknown as typeof ResizeObserver;
globalThis.matchMedia ??= ((q: string) => ({ matches: false, media: q, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {}, onchange: null, dispatchEvent: () => false })) as unknown as typeof matchMedia;
globalThis.requestAnimationFrame ??= ((cb: FrameRequestCallback) => setTimeout(() => cb(0), 0)) as unknown as typeof requestAnimationFrame;
afterEach(() => cleanup());
