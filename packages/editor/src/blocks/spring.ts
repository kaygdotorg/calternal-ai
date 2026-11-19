/**
 * One spring for the block-selection look (#397): critically damped, so it never overshoots,
 * and settled in about 250 ms. `OMEGA = 37` is the app's `spring(1, 0.17)`:
 * (1 + ωt)e^(-ωt) is under 0.001 at t = 250 ms.
 */
export const OMEGA = 37

export interface Spring {
  pos: number
  vel: number
}

/** Moves `s` toward `target` by `dt` seconds of the spring (the exact solution, so any dt is safe). */
export function step(s: Spring, target: number, dt: number): void {
  const x = s.pos - target
  const e = Math.exp(-OMEGA * dt)
  const c = s.vel + OMEGA * x
  s.pos = target + (x + c * dt) * e
  s.vel = (s.vel - OMEGA * c * dt) * e
}

export const settled = (s: Spring, target: number): boolean => Math.abs(s.pos - target) < 0.05 && Math.abs(s.vel) < 0.5

/** Runs `s` toward `target` on animation frames, calling `draw` each frame; returns a stop function. */
export function run(s: Spring, target: () => number, draw: (pos: number) => void, done?: () => void): () => void {
  let raf = 0
  let last = performance.now()
  const tick = (t: number) => {
    step(s, target(), Math.min((t - last) / 1000, 0.05))
    last = t
    if (settled(s, target())) {
      s.pos = target()
      s.vel = 0
      draw(s.pos)
      raf = 0
      done?.()
      return
    }
    draw(s.pos)
    raf = requestAnimationFrame(tick)
  }
  raf = requestAnimationFrame(tick)
  return () => cancelAnimationFrame(raf)
}
