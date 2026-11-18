/**
 * A damped spring stepped by hand, for motion that has to know its own speed while it runs (the switch's knob stretches with it) and that a finger can take hold of: no animation library, a few lines.
 * `step` moves `value` toward a target by `dt` seconds with semi-implicit Euler in sub-steps of at most 1/240 s, so a long frame does not blow the integration up. Units are the caller's (px, px per second).
 */
export interface SpringConfig { stiffness: number; damping: number; mass: number }

/** One value on a damped spring: `value` and `velocity`, advanced with `step`; the caller decides the target each frame and when it has arrived (`atRest`). */
export class Spring {
  value: number;
  velocity = 0;
  constructor(value: number, readonly config: SpringConfig) { this.value = value; }
  /** Advances by `dt` seconds toward `target`. */
  step(target: number, dt: number): void {
    const { stiffness: k, damping: c, mass: m } = this.config;
    let left = Math.min(dt, 0.25); // sub-steps cover a long frame, so a busy machine slows the picture, not the motion
    while (left > 1e-9) {
      const h = Math.min(left, 1 / 240);
      this.velocity += ((-k * (this.value - target) - c * this.velocity) / m) * h;
      this.value += this.velocity * h;
      left -= h;
    }
  }
  /** Whether it is close enough to `target` and slow enough to be called arrived. */
  atRest(target: number, delta = 0.05, speed = 0.5): boolean { return Math.abs(this.value - target) < delta && Math.abs(this.velocity) < speed; }
}
