// jsdom lacks these; the components only need them to exist.
globalThis.ResizeObserver ??= class { observe() {} unobserve() {} disconnect() {} } as unknown as typeof ResizeObserver;
globalThis.matchMedia ??= ((q: string) => ({ matches: false, media: q, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {}, onchange: null, dispatchEvent: () => false })) as unknown as typeof matchMedia;

import { afterEach } from "vitest";
import { cleanup } from "@testing-library/svelte";
afterEach(() => cleanup());
