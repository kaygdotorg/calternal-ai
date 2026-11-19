/**
 * Fits column widths into the card by wrapping text instead of scrolling.
 *
 * A column has the width its text wants (`wanted`) and the least it can take (`floor`: the longest word plus
 * padding, or the numbers' own width). When the wanted widths are wider than the card, each column gives back
 * a share of the width above its floor, in proportion to how much it has above it, so a long first column
 * wraps its text while a short number column keeps its size. If even the floors are wider than the card the
 * table stays at the floors and scrolls sideways. Pure arithmetic: no DOM, no editor.
 */
export function shrinkToFit(wanted: readonly number[], floor: readonly number[], width: number): number[] {
  const total = wanted.reduce((a, b) => a + b, 0)
  if (total <= width) return wanted.slice()
  const slack = wanted.map((w, c) => Math.max(0, w - Math.min(w, floor[c] ?? 0)))
  const spare = slack.reduce((a, b) => a + b, 0)
  if (spare <= 0) return wanted.slice()
  const take = Math.min(total - width, spare)
  return wanted.map((w, c) => w - (take * slack[c]!) / spare)
}
