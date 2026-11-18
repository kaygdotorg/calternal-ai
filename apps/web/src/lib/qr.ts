// A small QR code encoder for what Calternal shows as a code: a link, in byte mode, error correction M, versions 1 to 10
// (up to 213 bytes). A longer text gives no code ("" from `qrSvg`). Written instead of the general `qrcode-generator` library
// (21 KB, with number, letter, kanji modes and every size) that cost more than this whole feature: same bits as it, checked by
// e2e/qr.spec.ts against the library on many texts.

// Per version: [error words per block, blocks of the short size, their data words, blocks one data word longer].
const BLOCKS = [[10, 1, 16, 0], [16, 1, 28, 0], [26, 1, 44, 0], [18, 2, 32, 0], [24, 2, 43, 0], [16, 4, 27, 0], [18, 4, 31, 0], [22, 2, 38, 2], [22, 3, 36, 2], [26, 4, 43, 1]]
// Where the small squares sit (their centres, on rows and columns), from version 2.
const ALIGN = [[], [6, 18], [6, 22], [6, 26], [6, 30], [6, 34], [6, 22, 38], [6, 24, 42], [6, 26, 46], [6, 28, 50]]

// Numbers in the field the error words live in (x^8 + x^4 + x^3 + x^2 + 1).
const EXP: number[] = []
const LOG: number[] = []
for (let i = 0, x = 1; i < 255; i++, x = x & 128 ? (x << 1) ^ 285 : x << 1) LOG[(EXP[i] = x)] = i

/** The error words of one block of data words. */
function errorWords(data: number[], n: number) {
  let gen = [1]
  for (let i = 0; i < n; i++) {
    const next = new Array(gen.length + 1).fill(0)
    gen.forEach((g, j) => {
      next[j] ^= g
      next[j + 1] ^= g && EXP[(LOG[g]! + i) % 255]!
    })
    gen = next
  }
  const rest = [...data, ...new Array(n).fill(0)]
  for (let i = 0; i < data.length; i++) {
    const lead = rest[i]!
    if (lead) gen.forEach((g, j) => (rest[i + j] ^= g && EXP[(LOG[g]! + LOG[lead]!) % 255]!))
  }
  return rest.slice(data.length)
}

/** Format and version words: the value with a BCH remainder under it. */
function bch(value: number, poly: number, bits: number) {
  const len = (n: number) => 32 - Math.clz32(n)
  let v = value << bits
  while (len(v) >= len(poly)) v ^= poly << (len(v) - len(poly))
  return (value << bits) | v
}

const MASKS = [
  (i: number, j: number) => (i + j) % 2 == 0,
  (i: number) => i % 2 == 0,
  (_: number, j: number) => j % 3 == 0,
  (i: number, j: number) => (i + j) % 3 == 0,
  (i: number, j: number) => (((i / 2) | 0) + ((j / 3) | 0)) % 2 == 0,
  (i: number, j: number) => ((i * j) % 2) + ((i * j) % 3) == 0,
  (i: number, j: number) => (((i * j) % 2) + ((i * j) % 3)) % 2 == 0,
  (i: number, j: number) => (((i * j) % 3) + ((i + j) % 2)) % 2 == 0,
]

/** The code as rows of dark (true) and light cells, or null when the text does not fit. */
export function qrMatrix(text: string): boolean[][] | null {
  const bytes = [...new TextEncoder().encode(text)]
  const v = BLOCKS.findIndex(([, a, ad, b], i) => 4 + (i < 9 ? 8 : 16) + bytes.length * 8 <= 8 * (a! * ad! + b! * (ad! + 1))) + 1
  if (!v) return null
  const [ec, a, ad, b] = BLOCKS[v - 1]!
  const total = a! * ad! + b! * (ad! + 1)
  // Mode (byte), length, bytes, end, padding.
  let bits = '0100' + bytes.length.toString(2).padStart(v < 10 ? 8 : 16, '0') + bytes.map((x) => x.toString(2).padStart(8, '0')).join('')
  bits += '0000'.slice(0, Math.min(4, total * 8 - bits.length))
  bits += '0'.repeat((8 - (bits.length % 8)) % 8)
  const words = bits.match(/.{8}/g)!.map((x) => parseInt(x, 2))
  for (let i = 0; words.length < total; i++) words.push(i % 2 ? 0x11 : 0xec)
  // Cut into blocks, add each one's error words, then lay the words side by side.
  const blocks: number[][] = []
  let at = 0
  for (let i = 0; i < a! + b!; i++) blocks.push(words.slice(at, (at += ad! + (i < a! ? 0 : 1))))
  const errs = blocks.map((d) => errorWords(d, ec!))
  const out: number[] = []
  for (let i = 0; i <= ad!; i++) for (const d of blocks) if (i < d.length) out.push(d[i]!)
  for (let i = 0; i < ec!; i++) for (const e of errs) out.push(e[i]!)

  const n = v * 4 + 17
  const draw = (mask: number, test: boolean) => {
    const m: (boolean | null)[][] = Array.from({ length: n }, () => new Array(n).fill(null))
    const probe = (r0: number, c0: number) => {
      for (let r = -1; r <= 7; r++)
        for (let c = -1; c <= 7; c++)
          if (m[r0 + r]?.[c0 + c] !== undefined) m[r0 + r]![c0 + c] = (r >= 0 && r <= 6 && (c == 0 || c == 6)) || (c >= 0 && c <= 6 && (r == 0 || r == 6)) || (r >= 2 && r <= 4 && c >= 2 && c <= 4)
    }
    probe(0, 0), probe(n - 7, 0), probe(0, n - 7)
    for (const r of ALIGN[v - 1]!)
      for (const c of ALIGN[v - 1]!)
        if (m[r]![c] == null) for (let i = -2; i <= 2; i++) for (let j = -2; j <= 2; j++) m[r + i]![c + j] = Math.max(Math.abs(i), Math.abs(j)) != 1
    for (let i = 8; i < n - 8; i++) m[i]![6] ??= i % 2 == 0, m[6]![i] ??= i % 2 == 0
    const info = bch(mask, 0x537, 10) ^ 0x5412 // level M is 00
    for (let i = 0; i < 15; i++) {
      const d = !test && ((info >> i) & 1) == 1
      m[i < 6 ? i : i < 8 ? i + 1 : n - 15 + i]![8] = d
      m[8]![i < 8 ? n - i - 1 : i < 9 ? 15 - i : 14 - i] = d
    }
    m[n - 8]![8] = !test
    if (v >= 7) {
      const ver = bch(v, 0x1f25, 12)
      for (let i = 0; i < 18; i++) m[(i / 3) | 0]![(i % 3) + n - 11] = m[(i % 3) + n - 11]![(i / 3) | 0] = !test && ((ver >> i) & 1) == 1
    }
    // The words, in pairs of columns from the bottom right, up and down.
    let up = -1
    let r = n - 1
    let k = 0
    for (let c = n - 1; c > 0; c -= 2) {
      if (c == 6) c--
      for (;;) {
        for (const x of [c, c - 1])
          if (m[r]![x] == null) {
            m[r]![x] = (((out[k >> 3] ?? 0) >> (7 - (k & 7))) & 1) == 1 !== MASKS[mask]!(r, x)
            k++
          }
        r += up
        if (r < 0 || r >= n) {
          r -= up
          up = -up
          break
        }
      }
    }
    return m as boolean[][]
  }
  // The mask that leaves the code the least patterned (same rules, same ties, as the library).
  let best = 0
  let least = Infinity
  for (let mask = 0; mask < 8; mask++) {
    const s = lost(draw(mask, true))
    if (s < least) (least = s), (best = mask)
  }
  return draw(best, false)
}

function lost(m: boolean[][]) {
  const n = m.length
  let s = 0
  let dark = 0
  for (let r = 0; r < n; r++)
    for (let c = 0; c < n; c++) {
      let same = 0
      for (let i = -1; i <= 1; i++) for (let j = -1; j <= 1; j++) if ((i || j) && m[r + i]?.[c + j] === m[r]![c]) same++
      if (same > 5) s += same - 2
      if (m[r]![c]) dark++
      if (r < n - 1 && c < n - 1) {
        const k = +m[r]![c]! + +m[r + 1]![c]! + +m[r]![c + 1]! + +m[r + 1]![c + 1]!
        if (k == 0 || k == 4) s += 3
      }
      // Dark, light, dark dark dark, light, dark: a mark that looks like a corner square.
      for (const [dr, dc] of [[0, 1], [1, 0]] as const)
        if (r + 6 * dr < n && c + 6 * dc < n && [1, 0, 1, 1, 1, 0, 1].every((x, i) => +m[r + i * dr]![c + i * dc]! == x)) s += 40
    }
  return s + (Math.abs((100 * dark) / n / n - 50) / 5) * 10
}

/** The code as a scalable image, with a two-cell quiet border; "" when the text is too long. */
export function qrSvg(text: string) {
  const m = qrMatrix(text)
  if (!m) return ""
  const size = m.length + 4
  let d = ""
  m.forEach((row, r) => row.forEach((on, c) => on && (d += `M${c + 2},${r + 2}h1v1h-1z`)))
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${size} ${size}" shape-rendering="crispEdges"><rect width="100%" height="100%" fill="white"/><path d="${d}" fill="black"/></svg>`
}
