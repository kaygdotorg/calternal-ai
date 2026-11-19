/**
 * The pieces an embed is drawn with: a picture card, a video, a recording, a PDF card and a file row.
 * Plain elements with stable `cm-embed*` class names; the look lives in the app's stylesheet. Every box
 * has its final shape before anything loads (an aspect ratio from the file's own size, or a fixed
 * shape), pictures load lazily and decode off the main thread, and a video or a recording is a card
 * until someone plays it: nothing is downloaded for a player nobody uses.
 */

import { type EmbedItem, sizeWords, shownName } from './embeds'

/** A box shape when a picture's size is not known yet (the mockup's own: 3 by 2). */
const GUESS = { w: 3, h: 2 }

export function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag)
  e.className = cls
  if (text != null) e.textContent = text
  return e
}

/** Turns the label (or title) into a field on a double click; Enter keeps the words, Esc leaves them. */
export function editable(label: HTMLElement, current: string, custom: boolean, commit: (words: string) => void, reset: string): void {
  label.addEventListener('dblclick', (ev) => {
    ev.stopPropagation()
    ev.preventDefault()
    getSelection()?.removeAllRanges()
    const input = el('input', 'cm-embed-edit')
    input.value = current
    input.setAttribute('aria-label', 'Title')
    input.spellcheck = false
    let done = false
    const finish = (keep: boolean) => {
      if (done) return
      done = true
      const v = input.value.trim()
      input.replaceWith(label)
      if (keep && v !== current.trim()) commit(v)
    }
    input.addEventListener('keydown', (e) => {
      e.stopPropagation()
      if (e.key === 'Enter') (e.preventDefault(), finish(true))
      else if (e.key === 'Escape') (e.preventDefault(), finish(false))
    })
    input.addEventListener('click', (e) => e.stopPropagation())
    input.addEventListener('dblclick', (e) => e.stopPropagation())
    input.addEventListener('blur', () => finish(true))
    label.replaceWith(input)
    input.focus()
    input.select()
  })
  if (custom) label.title = reset
}

/** The card every embed sits on: a lifted print with a soft shadow. */
export function lift(...kids: Node[]): HTMLElement {
  const l = el('span', 'cm-embed-lift bounce')
  l.append(...kids)
  return l
}

/** The name on the picture's lower edge, with the facts that matter on the right. */
function strip(i: EmbedItem, facts: string[], rename?: (words: string) => void): HTMLElement {
  const s = el('span', 'cm-embed-strip')
  const label = el('span', 'cm-embed-label', shownName(i))
  if (rename) editable(label, shownName(i), !!i.alt.trim(), rename, "Double-click to change the title")
  s.append(label)
  const f = el('span', 'cm-embed-facts')
  for (const t of facts) f.append(el('span', '', t))
  s.append(f)
  return s
}

function pictureFacts(i: EmbedItem): string[] {
  const { w, h, size } = i.source
  const out: string[] = []
  if (w && h) out.push(`${w} × ${h}`)
  if (size) out.push(sizeWords(size))
  return out
}

/** A picture (or GIF) as a card. A picture that cannot be drawn here (HEIC outside Safari) becomes a file row. */
export function pictureCard(i: EmbedItem, onBroken: () => void, rename?: (words: string) => void): HTMLElement {
  const { w = GUESS.w, h = GUESS.h } = i.source
  const clip = el('span', 'cm-embed-clip cm-image-clip')
  clip.style.aspectRatio = `${w} / ${h}`
  const img = el('img', 'cm-embed-photo cm-image-pic')
  img.alt = i.alt
  img.loading = 'lazy'
  img.decoding = 'async'
  img.draggable = false
  if (i.source.w && i.source.h) {
    img.width = i.source.w
    img.height = i.source.h
  }
  const { learned } = i.source
  if (learned && !(i.source.w && i.source.h)) {
    img.addEventListener(
      'load',
      () => {
        // The one time the box was a guess: it takes the picture's own shape, and is remembered.
        clip.style.aspectRatio = `${img.naturalWidth} / ${img.naturalHeight}`
        learned(img.naturalWidth, img.naturalHeight)
      },
      { once: true },
    )
  }
  img.addEventListener('error', onBroken, { once: true })
  img.src = i.source.src
  clip.append(img, strip(i, pictureFacts(i), rename))
  return lift(clip)
}

const clock = (s: number): string => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, '0')}`

/** A video: a still card with a play button; the player appears (and loads) on the first tap. */
export function videoCard(i: EmbedItem, rename?: (words: string) => void): HTMLElement {
  const clip = el('span', 'cm-embed-clip cm-embed-video-clip')
  const play = el('button', 'cm-embed-play bounce')
  play.type = 'button'
  play.setAttribute('aria-label', `Play ${shownName(i)}`)
  play.innerHTML = '<svg width="26" height="26" viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M8 5.5v13a.5.5 0 0 0 .76.43l10.4-6.5a.5.5 0 0 0 0-.86L8.76 5.07A.5.5 0 0 0 8 5.5z"/></svg>'
  const facts = i.source.size ? [sizeWords(i.source.size)] : []
  const bar = strip(i, facts, rename)
  play.addEventListener('click', (ev) => {
    ev.stopPropagation()
    clip.closest('.cm-embed')?.setAttribute('data-playing', '')
    const v = el('video', 'cm-embed-photo cm-embed-video')
    v.src = i.source.src
    v.controls = true
    v.playsInline = true
    v.autoplay = true
    v.preload = 'auto'
    play.remove()
    bar.remove()
    clip.append(v)
    void v.play().catch(() => {})
  })
  clip.append(play, bar)
  return lift(clip)
}

const BARS = 48

/** A recording: a round play button, its name and a row of bars that fill as it plays; tap them to jump. */
export function audioCard(i: EmbedItem): HTMLElement {
  const card = el('span', 'cm-embed-audio')
  const top = el('span', 'cm-embed-audio-top')
  const pp = el('button', 'cm-embed-pp bounce')
  pp.type = 'button'
  const PLAY = '<svg width="20" height="20" viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M8 5.5v13a.5.5 0 0 0 .76.43l10.4-6.5a.5.5 0 0 0 0-.86L8.76 5.07A.5.5 0 0 0 8 5.5z"/></svg>'
  const PAUSE = '<svg width="20" height="20" viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M7 5h3.5v14H7zM13.5 5H17v14h-3.5z"/></svg>'
  pp.innerHTML = PLAY
  pp.setAttribute('aria-label', `Play ${shownName(i)}`)
  const names = el('span', 'cm-embed-names')
  const meta = el('span', 'cm-embed-meta', ['Recording', i.source.size ? sizeWords(i.source.size) : ''].filter(Boolean).join(' · '))
  names.append(el('span', 'cm-embed-name', shownName(i)), meta)
  top.append(pp, names)
  const wave = el('span', 'cm-embed-wave')
  wave.setAttribute('role', 'slider')
  wave.setAttribute('aria-label', 'Position')
  const spans = Array.from({ length: BARS }, () => wave.appendChild(el('span', '')))
  card.append(top, wave)

  let audio: HTMLAudioElement | null = null
  const paint = () => {
    if (!audio) return
    const done = audio.duration ? audio.currentTime / audio.duration : 0
    spans.forEach((s, n) => s.classList.toggle('played', n / BARS < done))
    meta.textContent = `${clock(audio.currentTime)}${audio.duration ? ` / ${clock(audio.duration)}` : ''}`
  }
  const ensure = (): HTMLAudioElement => {
    if (audio) return audio
    audio = new Audio()
    audio.preload = 'auto'
    audio.src = i.source.src
    audio.addEventListener('timeupdate', paint)
    audio.addEventListener('loadedmetadata', paint)
    audio.addEventListener('play', () => ((pp.innerHTML = PAUSE), pp.setAttribute('aria-label', 'Pause')))
    const rest = () => ((pp.innerHTML = PLAY), pp.setAttribute('aria-label', `Play ${shownName(i)}`))
    audio.addEventListener('pause', rest)
    audio.addEventListener('ended', rest)
    void decodeBars(i.source.src).then((peaks) => peaks && spans.forEach((s, n) => s.style.setProperty('--h', String(peaks[n]))))
    return audio
  }
  pp.addEventListener('click', (ev) => {
    ev.stopPropagation()
    card.closest('.cm-embed')?.setAttribute('data-playing', '')
    const a = ensure()
    if (a.paused) void a.play().catch(() => {})
    else a.pause()
  })
  wave.addEventListener('click', (ev) => {
    ev.stopPropagation()
    const a = ensure()
    const r = wave.getBoundingClientRect()
    const to = Math.max(0, Math.min(1, (ev.clientX - r.left) / r.width))
    const go = () => ((a.currentTime = to * (a.duration || 0)), void a.play().catch(() => {}))
    if (a.duration) go()
    else a.addEventListener('loadedmetadata', go, { once: true })
  })
  return lift(card)
}

/** The loudness of each slice of the sound, read once it is first played. Null when it cannot be decoded. */
async function decodeBars(src: string): Promise<number[] | null> {
  try {
    const buf = await (await fetch(src, { credentials: 'same-origin' })).arrayBuffer()
    const audio = await new OfflineAudioContext(1, 1, 44100).decodeAudioData(buf)
    const data = audio.getChannelData(0)
    const step = Math.max(1, Math.floor(data.length / BARS))
    const out: number[] = []
    for (let n = 0; n < BARS; n++) {
      let sum = 0
      for (let j = n * step; j < (n + 1) * step && j < data.length; j++) sum += data[j]! * data[j]!
      out.push(Math.sqrt(sum / step))
    }
    const max = Math.max(...out) || 1
    return out.map((v) => Math.max(0.08, v / max))
  } catch {
    return null
  }
}

const HUE: Record<string, string> = { PDF: '#c0392b', ZIP: '#7a6a58', CSV: '#2e8b57', PY: '#3b6db3', KEY: '#c26a12' }

/** A file as a row: a coloured tile with its type, its name, its size and kind in words. */
export function fileRow(i: EmbedItem, kindWord: string, card: boolean): HTMLElement {
  const ext = (/\.([a-z0-9]+)$/i.exec(i.source.name)?.[1] ?? '').toUpperCase().slice(0, 4)
  const hue = HUE[ext] ?? '#8a7f74'
  const row = el('span', card ? 'cm-embed-file cm-embed-doc' : 'cm-embed-file')
  const tile = el('span', 'cm-embed-tile', ext || 'FILE')
  tile.style.setProperty('--tile-bg', `color-mix(in oklab, ${hue} 15%, transparent)`)
  tile.style.setProperty('--tile-ink', hue)
  const names = el('span', 'cm-embed-names')
  const meta = [kindWord, i.source.size ? sizeWords(i.source.size) : '', i.source.pages ? `${i.source.pages} page${i.source.pages === 1 ? '' : 's'}` : '']
  names.append(el('span', 'cm-embed-name', shownName(i)), el('span', 'cm-embed-meta', meta.filter(Boolean).join(' · ')))
  row.append(tile, names)
  return lift(row)
}
