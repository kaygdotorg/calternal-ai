/**
 * A Fenwick (binary indexed) tree over row heights: the sum of the first `i` rows and "which row holds this offset" are both
 * O(log n), and changing one row's height is O(log n). That is what lets a list of ten thousand rows with unknown heights scroll
 * without ever summing all of them.
 */
export class Fenwick {
  /** Number of rows. */
  readonly n: number;
  private readonly t: Float64Array;
  private readonly top: number;

  /** Builds the tree from `heights` (one per row) in O(n). */
  constructor(heights: ArrayLike<number>) {
    this.n = heights.length;
    this.t = new Float64Array(this.n + 1);
    for (let i = 1; i <= this.n; i++) this.t[i] = heights[i - 1]!;
    for (let i = 1; i <= this.n; i++) {
      const j = i + (i & -i);
      if (j <= this.n) this.t[j] = this.t[j]! + this.t[i]!;
    }
    this.top = this.n ? 2 ** Math.floor(Math.log2(this.n)) : 0;
  }

  /** Adds `delta` to row `i`'s height. O(log n). */
  add(i: number, delta: number): void {
    for (let k = i + 1; k <= this.n; k += k & -k) this.t[k] = this.t[k]! + delta;
  }

  /** Total height of rows `[0, i)`. O(log n). */
  prefix(i: number): number {
    let s = 0;
    for (let k = Math.min(i, this.n); k > 0; k -= k & -k) s += this.t[k]!;
    return s;
  }

  /** The row that contains vertical `offset` (clamped to the first and last row). O(log n). */
  find(offset: number): number {
    if (this.n === 0) return 0;
    let pos = 0;
    let rest = offset;
    for (let step = this.top; step > 0; step >>= 1) {
      const next = pos + step;
      if (next <= this.n && this.t[next]! <= rest) {
        pos = next;
        rest -= this.t[next]!;
      }
    }
    return Math.min(pos, this.n - 1);
  }
}
