// Ported from the existing app (origin/dev packages/ui/src/primitives/toast), logic and motion unchanged except where noted.
/** Where a toast is shown: the stack draws them and times them. (The existing app also has a presenter for its bottom bar; this shell has no bottom bar, so only the stack exists.) */
export type Presenter = {
  /** Show this toast (or show it again, changed) for `duration` ms from when it's visible. */
  show(id: string, duration: number): void;
  dismiss(id: string): void;
};
