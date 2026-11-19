/**
 * Keeps the bar capsule (Morph.svelte) the size of the layer it shows NOW.
 *
 * Why it exists: the capsule used to take its size only from a ResizeObserver callback on the new layer.
 * When a sheet closed, that callback could be skipped (it is dropped when the key no longer matches, and
 * the observer's first call may land late or never when the editor is busy filling a slot), and the capsule
 * stayed at the sheet's size over the page. Here the layer is measured on purpose: once when it mounts and
 * once in the next frame, besides every observer call. A call that arrives for a layer that is no longer
 * the current one is dropped, so a late callback can never put an old size back.
 *
 * The capsule also looks up the current layer by its key whenever the key changes (`currentLayer`): in the e2e
 * failure the new layer was in the page but its action had not run, so no call of its own ever came.
 *
 * Cost: a few reads of the layer's size per key change, nothing per frame.
 */

export type Size = { w: number; h: number };

/** The parts of a layer element that are read. */
export interface LayerEl {
  dataset: DOMStringMap;
  offsetWidth: number;
  offsetHeight: number;
  isConnected: boolean;
}

/** What the sizer needs from its host; the defaults are the browser's. */
export interface SizerDeps {
  /** The key of the layer the capsule should show right now. */
  key: () => string;
  /** The size the capsule has now, or null before its first. */
  size: () => Size | null;
  /** Makes `next` the capsule's size. Called only when it differs from `size()`. */
  apply: (next: Size) => void;
  observe?: (el: LayerEl, call: () => void) => () => void;
  frame?: (call: () => void) => () => void;
}

const observeResize = (el: LayerEl, call: () => void) => {
  const ro = new ResizeObserver(call);
  ro.observe(el as unknown as Element);
  return () => ro.disconnect();
};
const nextFrame = (call: () => void) => {
  const id = requestAnimationFrame(call);
  return () => cancelAnimationFrame(id);
};

/** Reads `el` and makes it the capsule's size, unless it is not the current layer or the size is the same. */
export function measureLayer(el: LayerEl, deps: Pick<SizerDeps, "key" | "size" | "apply">): void {
  // A layer that has been replaced (or removed) must not set the size: its call is late.
  if (!el.isConnected || el.dataset.key !== deps.key()) return;
  const next = { w: el.offsetWidth, h: el.offsetHeight };
  const was = deps.size();
  if (was && was.w === next.w && was.h === next.h) return;
  deps.apply(next);
}

/** Follows `el` until `stop()`: measures it now, on each observer call, and once in the next frame. */
export function followLayer(el: LayerEl, deps: SizerDeps): { sync: () => void; stop: () => void } {
  const sync = () => measureLayer(el, deps);
  const stopObserving = (deps.observe ?? observeResize)(el, sync);
  const stopFrame = (deps.frame ?? nextFrame)(sync);
  sync();
  return {
    sync,
    stop: () => {
      stopObserving();
      stopFrame();
    },
  };
}

/** The layer that shows `key` now: the last one with that key (a replaced one can still be on its way out). */
export function currentLayer(parent: { children: ArrayLike<unknown> } | undefined, key: string): LayerEl | undefined {
  const all = Array.from((parent?.children ?? []) as ArrayLike<LayerEl>);
  return all.reverse().find((l) => l.dataset.key === key);
}
