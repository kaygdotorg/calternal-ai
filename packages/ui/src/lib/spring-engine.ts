// Springs you can grab mid-flight. Each one keeps its own value and velocity, so a new target
// retargets from wherever it is right now: no snap back, no restart from zero. One animation
// frame loop drives every running spring and stops as soon as they've all settled, so nothing
// ticks while the screen is still.
//
// Parameters follow Apple's model: `damping` is the damping ratio (1 = no overshoot, lower =
// more bounce) and `response` is roughly the seconds it takes to get there.

export type SpringConfig = { damping: number; response: number };

export class Spring {
  value: number;
  velocity = 0;
  target: number;
  config: SpringConfig;
  /** Close enough to call it done, in the value's own units. */
  precision: number;

  constructor(value: number, config: SpringConfig, precision = 0.01) {
    this.value = value;
    this.target = value;
    this.config = config;
    this.precision = precision;
  }

  /** Jumps straight to a value, at rest (first render, keyboard, reduced motion). */
  snap(value: number) {
    this.value = value;
    this.target = value;
    this.velocity = 0;
  }

  get settled(): boolean {
    return Math.abs(this.target - this.value) < this.precision && Math.abs(this.velocity) < this.precision * 10;
  }

  /** Advances by `dt` seconds (semi-implicit Euler in small fixed steps, so it stays stable). */
  step(dt: number) {
    const { damping, response } = this.config;
    const k = (2 * Math.PI / response) ** 2; // stiffness, for unit mass
    const c = (4 * Math.PI * damping) / response; // damping coefficient
    const h = 1 / 240;
    for (let t = 0; t < dt; t += h) {
      const s = Math.min(h, dt - t);
      const force = -k * (this.value - this.target) - c * this.velocity;
      this.velocity += force * s;
      this.value += this.velocity * s;
    }
    if (this.settled) this.snap(this.target);
  }
}

type Job = { springs: Spring[]; frame: () => void; done?: () => void };
const jobs = new Set<Job>();
let raf = 0;
let last = 0;

/**
 * The most time one frame may advance a spring, in seconds. A spring takes the same time on the clock at any frame rate: a frame that
 * came late (a slow machine, software drawing under load) advances it by the whole gap, in `step`'s small fixed sub-steps, so it
 * stays stable. The cap is only for a gap of minutes (a background tab): longer than any spring here needs to come to rest, so it
 * lands at rest, and the loop never runs for minutes of sub-steps. (It was 1/20 s, which made every animation 20 times slower than
 * the clock on a machine drawing one frame a second: the toast's Undo stayed half open for half a minute.)
 */
const MAX_FRAME_S = 2;

function tick(now: number) {
  const dt = Math.min(MAX_FRAME_S, (now - last) / 1000);
  last = now;
  for (const job of [...jobs]) {
    for (const s of job.springs) s.step(dt);
    job.frame();
    if (job.springs.every((s) => s.settled)) {
      jobs.delete(job);
      job.done?.();
    }
  }
  raf = jobs.size ? requestAnimationFrame(tick) : 0;
}

/**
 * Runs `frame` every animation frame until all `springs` settle (then `done`). Returns a stop
 * function. Calling it again for springs that are already moving just keeps them moving: their
 * velocity carries over.
 */
export function run(springs: Spring[], frame: () => void, done?: () => void): () => void {
  const job: Job = { springs, frame, done };
  jobs.add(job);
  if (!raf) {
    last = performance.now();
    raf = requestAnimationFrame(tick);
  }
  return () => jobs.delete(job);
}

/** Is any job currently animating? (For tests: nothing should tick once everything is still.) */
export const active = () => jobs.size > 0;

export const prefersReducedMotion = () =>
  typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

