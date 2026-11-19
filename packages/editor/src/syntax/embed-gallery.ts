/**
 * A gallery: two or more pictures on one line. Stack (the default): bounce cards, a fan of prints.
 * Pointing at one brings it forward and pushes the others aside; a tap picks it and opens it. Grid
 * (`"grid"` after the first picture's address): every picture, filling the width in even rows.
 *
 * Reimplemented from sveltebits' bounce-cards: the same fan, timing and curves, without GSAP, and
 * scaled by the card's width (the original's numbers are for a 200 px card).
 */

import { el } from './embed-dom'
import { type EmbedItem, shownName } from './embeds'

const FAN = [
  { rot: 10, x: -170 },
  { rot: 5, x: -85 },
  { rot: -3, x: 0 },
  { rot: -10, x: 85 },
  { rot: 2, x: 170 },
]
const PUSH = 160
const INTRO = { delay: 500, stagger: 60, duration: 300 }
const MOVE_MS = 400

/** GSAP's elastic.out and back.out as CSS `linear()` easings. */
function elasticOut(a: number, p: number, steps = 80): string {
  const p3 = (p / (2 * Math.PI)) * Math.asin(1 / a)
  const f = (t: number) => (t === 0 ? 0 : t === 1 ? 1 : a * Math.pow(2, -10 * t) * Math.sin((t - p3) * ((2 * Math.PI) / p)) + 1)
  return `linear(${Array.from({ length: steps + 1 }, (_, i) => f(i / steps).toFixed(4)).join(', ')})`
}
function backOut(s: number, steps = 40): string {
  const f = (t: number) => 1 + (s + 1) * Math.pow(t - 1, 3) + s * Math.pow(t - 1, 2)
  return `linear(${Array.from({ length: steps + 1 }, (_, i) => f(i / steps).toFixed(4)).join(', ')})`
}

/** The fan for n cards: the original's five places, evened out for other counts. */
function fan(n: number) {
  if (n === FAN.length) return FAN
  if (n < FAN.length) {
    const start = Math.floor((FAN.length - n) / 2)
    const part = FAN.slice(start, start + n)
    const mid = part.reduce((a, p) => a + p.x, 0) / n
    return part.map((p) => ({ rot: p.rot, x: p.x - mid }))
  }
  return Array.from({ length: n }, (_, i) => ({ rot: FAN[i % FAN.length]!.rot, x: -170 + (340 * i) / (n - 1) }))
}

/** A card's transform the way GSAP writes it: a screen-space move, then the turn. */
const place = (rot: number, x: number, u: number): string => {
  const r = (rot * Math.PI) / 180
  return `translate(${(x * u * Math.cos(r)).toFixed(2)}px, ${(x * u * Math.sin(r)).toFixed(2)}px) rotate(${rot}deg)`
}

const picture = (i: EmbedItem): HTMLImageElement => {
  const img = el('img', 'cm-embed-photo')
  img.alt = i.alt
  img.loading = 'lazy'
  img.decoding = 'async'
  img.draggable = false
  img.src = i.source.src
  return img
}

/** The gallery; `open(index)` is called when a picture is tapped. `room` is the line's width, so the fan is the right size from the first frame. */
export function gallery(items: EmbedItem[], grid: boolean, open: (index: number) => void, room: number): HTMLElement {
  const root = el('span', 'cm-embed-gallery')
  root.dataset.layout = grid ? 'grid' : 'stack'
  root.setAttribute('role', 'group')
  root.setAttribute('aria-label', `Gallery, ${items.length} photos`)
  if (grid) {
    root.dataset.n = String(items.length)
    if (items.length % 2 === 1 && items.length % 3 !== 0) root.classList.add('odd')
    items.forEach((it, i) => {
      const cell = el('span', 'cm-embed-cell')
      cell.append(picture(it))
      cell.addEventListener('click', () => open(i))
      root.append(cell)
    })
    return root
  }
  const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches
  const stack = el('span', 'cm-embed-stack')
  stack.style.setProperty('--ease', backOut(1.4))
  stack.style.setProperty('--ms', `${MOVE_MS}ms`)
  const spots = fan(items.length)
  let front = Math.floor(items.length / 2)
  let hovered: number | null = null
  let picked = false
  let width = room > 0 ? room : 600
  const cards = items.map((it, i) => {
    const c = el('span', 'cm-embed-card')
    c.setAttribute('role', 'group')
    c.setAttribute('aria-label', `${i + 1} of ${items.length}: ${shownName(it)}`)
    c.append(picture(it))
    c.addEventListener('pointerenter', (e) => {
      if (e.pointerType !== 'mouse') return
      front = i
      hovered = i
      layout()
    })
    c.addEventListener('click', () => {
      front = i
      picked = true
      layout()
      open(i)
    })
    stack.append(c)
    return c
  })
  const foot = el('span', 'cm-embed-foot')
  const alt = el('span', 'cm-embed-foot-alt')
  const count = el('span', 'cm-embed-foot-count')
  foot.append(alt, count)

  function layout() {
    const w = Math.round(Math.min(200, width / 2.75))
    const u = w / 200
    stack.style.setProperty('--w', `${w}px`)
    const posed = hovered ?? (picked ? front : null)
    cards.forEach((c, i) => {
      const s = spots[i]!
      let t = place(s.rot, s.x, u)
      if (posed !== null && i === posed) t = place(0, s.x, u)
      else if (posed !== null && hovered !== null) t = place(s.rot, s.x + (i < posed ? -PUSH : PUSH), u)
      c.style.transform = t
      c.style.transitionDelay = hovered === null ? '0ms' : `${Math.abs(hovered - i) * 50}ms`
      c.classList.toggle('posed', i === posed)
      c.classList.toggle('lifted', picked && i === front)
    })
    alt.textContent = shownName(items[front]!)
    count.textContent = `${front + 1} of ${items.length}`
  }
  stack.addEventListener('pointerleave', () => {
    hovered = null
    layout()
  })
  layout()
  root.append(stack, foot)

  // The stack takes the line's width; the fan is drawn again when it changes. It bounces in once, when it
  // comes into view (reduced motion: it is simply there).
  if (typeof ResizeObserver !== 'undefined') {
    new ResizeObserver(([e]) => {
      const w = e?.contentRect.width ?? 0
      if (w && Math.abs(w - width) > 1) {
        width = w
        layout()
      }
    }).observe(stack)
  }
  if (!reduce && typeof IntersectionObserver !== 'undefined') {
    stack.classList.add('waiting')
    const ease = elasticOut(1, 0.8)
    const io = new IntersectionObserver(
      ([e]) => {
        if (!e?.isIntersecting) return
        io.disconnect()
        stack.classList.remove('waiting')
        cards.forEach((c, i) => {
          const t = c.style.transform
          c.animate([{ transform: `${t} scale(0)` }, { transform: `${t} scale(1)` }], {
            duration: INTRO.duration,
            delay: INTRO.delay + i * INTRO.stagger,
            easing: ease,
            fill: 'backwards',
          })
        })
      },
      { threshold: 0.45 },
    )
    io.observe(stack)
  }
  return root
}
