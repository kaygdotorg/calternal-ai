// Ported from the existing app (apps/web/src/rows/spring-to.ts on origin/dev, #400) without changes.
// A spring that starts moving: the easing for a release that keeps the finger's speed (#400).
// `spring()` in packages/ui always starts at rest; a flicked row must leave with the speed it was
// thrown at, or the hand-off from finger to spring shows a hitch. This builds a CSS `linear()`
// easing for "from here to there, starting at this speed", so the motion runs as a plain WAAPI
// transform on the compositor. Overshoot shows as values past 1, which `linear()` allows.

export type Release = { easing: string; duration: number };

/**
 * From `from` to `to` (px), leaving at `velocity` (px/ms, in screen direction).
 * `damping` 1 settles without overshoot; lower bounces. `response` is roughly seconds to arrive.
 */
export function springTo(from: number, to: number, velocity: number, damping = 0.86, response = 0.36): Release {
  const span = to - from;
  if (Math.abs(span) < 0.5) return { easing: "linear", duration: 1 };
  const w = (2 * Math.PI) / response;
  // Speed as a share of the distance per second, the unit the normalised curve runs in.
  const v0 = (velocity * 1000) / span;
  const at = (t: number) => {
    // Displacement from the target: starts at -1, with speed v0.
    if (damping >= 1) return 1 + (-1 + (v0 - w) * t) * Math.exp(-w * t);
    const wd = w * Math.sqrt(1 - damping * damping);
    const b = (v0 - damping * w) / wd;
    return 1 + Math.exp(-damping * w * t) * (-Math.cos(wd * t) + b * Math.sin(wd * t));
  };
  // Settled once it stays within half a pixel of the target.
  const tol = Math.min(0.01, 0.5 / Math.abs(span));
  let end = 0.05;
  for (let t = 0; t < 2; t += 0.004) if (Math.abs(1 - at(t)) > tol) end = t;
  const steps = 40;
  const points: string[] = [];
  for (let i = 0; i <= steps; i++) points.push((i === steps ? 1 : at((end * i) / steps)).toFixed(4));
  return { easing: `linear(${points.join(", ")})`, duration: Math.round(end * 1000) };
}
