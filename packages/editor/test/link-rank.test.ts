/** Picker correctness at small and 10k sizes; wall/CPU budgets run in perf/receipt-rank.ts. */
import { describe, expect, it } from 'vitest'
import { type TitleEntry, fold, rankLinks, rankTitles } from '../src/transforms/link-rank'
import { linkedFrom } from '../src/transforms/links'

let n = 0
const note = (title: string, edited = '2026-10-01T00:00:00.000Z', day?: string): TitleEntry => ({
  id: `n${++n}`,
  title,
  key: fold(title),
  day,
  path: `Notes/${title}.md`,
  edited,
})
const titles = (rows: { title: string }[]) => rows.map((r) => r.title)

describe('which notes the [[ picker offers by title (#448)', () => {
  const notes = [
    note('Keshav'),
    note("Keshav's trip"),
    note('Call with Keshav Nayyar'),
    note('Akeshav notes'),
    note('Kitchen Express hobby shop'),
    note('Friday, 1 May', '2026-05-01T00:00:00.000Z', '2026-05-01'),
    note('Thursday, 2 April', '2026-04-02T00:00:00.000Z', '2026-04-02'),
  ]

  it('puts the exact title first, then titles that start with it, then a word that does, then the rest', () => {
    expect(titles(rankTitles(notes, 'keshav'))).toEqual(['Keshav', "Keshav's trip", 'Call with Keshav Nayyar', 'Akeshav notes'])
  })

  it('finds titles by their letters in order when nothing else fits, from three letters on', () => {
    expect(titles(rankTitles(notes, 'kitchn'))).toEqual(['Kitchen Express hobby shop'])
    expect(titles(rankTitles(notes, 'kx'))).toEqual([])
    // Spread over the whole title is not close.
    expect(titles(rankTitles(notes, 'kes'))).toContain('Keshav')
    expect(rankTitles([note('Kind of every sort of hobby')], 'ksh')).toEqual([])
  })

  it('ignores case and Unicode form, and any order of the words', () => {
    expect(titles(rankTitles(notes, 'CALL KESHAV'))).toEqual(['Call with Keshav Nayyar'])
    expect(titles(rankTitles([note('Café ideas')], 'café'))).toEqual(['Café ideas'])
  })

  it('matches a day note by its title and by its date, never by its text', () => {
    expect(titles(rankTitles(notes, 'friday'))).toEqual(['Friday, 1 May'])
    expect(titles(rankTitles(notes, '2026-04'))).toEqual(['Thursday, 2 April'])
    expect(titles(rankTitles(notes, 'keshav'))).not.toContain('Friday, 1 May')
  })

  it('breaks ties by what the person links most, then the newest change, never the note itself', () => {
    const a = note('Trip to Goa', '2026-01-01T00:00:00.000Z')
    const b = note('Trip to Hampi', '2026-03-01T00:00:00.000Z')
    const c = note('Trip to Kochi', '2026-02-01T00:00:00.000Z')
    expect(titles(rankTitles([a, b, c], 'trip'))).toEqual(['Trip to Hampi', 'Trip to Kochi', 'Trip to Goa'])
    const lead = new Map([[a.id, 0], [c.id, 1]])
    expect(titles(rankTitles([a, b, c], 'trip', { lead }))).toEqual(['Trip to Goa', 'Trip to Kochi', 'Trip to Hampi'])
    expect(titles(rankTitles([a, b, c], 'trip', { skip: b.id }))).toEqual(['Trip to Kochi', 'Trip to Goa'])
    // A better tier beats use and recency.
    const exact = note('Trip', '2020-01-01T00:00:00.000Z')
    expect(titles(rankTitles([a, b, exact], 'trip', { lead: new Map([[a.id, 0]]) }))[0]).toBe('Trip')
  })

  it('keeps only the best few', () => {
    const many = Array.from({ length: 40 }, (_, i) => note(`Idea ${i}`, `2026-01-${String((i % 28) + 1).padStart(2, '0')}T00:00:00.000Z`))
    expect(rankTitles(many, 'idea', { limit: 6 })).toHaveLength(6)
  })

  it('puts titles the search matched with typos forgiven after every title we can match', () => {
    const kes = note('Keshav Nayyar')
    const rows = rankLinks({ query: 'keshve', notes: [kes, note('Keshve Café')], forgiven: [kes] })
    expect(titles(rows.filter((r) => r.kind === 'note'))).toEqual(['Keshve Café', 'Keshav Nayyar'])
  })
})

describe('the rows of the [[ picker, in order (#448)', () => {
  const day = note('Friday, 1 May', '2026-05-01T00:00:00.000Z', '2026-05-01')
  const mention = (title: string, id: string) => ({ kind: 'mention' as const, id, title, path: `Notes/${title}.md`, snippet: { text: 'met Keshav Nayyar for lunch', marks: [[4, 17]] as [number, number][] } })
  const createAt = (t: string) => `Notes/${t}.md`
  const nayyar = { title: 'Keshav Nayyar', path: 'Notes/Keshav Nayyar.md', count: 2 }

  it('offers a target linked before but never made, with how many notes link to it, and not the days that mention it', () => {
    const rows = rankLinks({ query: 'Keshav Nayyar', notes: [day], missing: [nayyar], mentions: [mention('Friday, 1 May', day.id)], createAt })
    expect(rows.map((r) => [r.kind, r.title])).toEqual([
      ['missing', 'Keshav Nayyar'],
      // Few titles match, so the days that only mention it follow, apart.
      ['mention', 'Friday, 1 May'],
    ])
    expect(rows[0]).toMatchObject({ count: 2, path: 'Notes/Keshav Nayyar.md' })
  })

  it('puts titles, then targets nobody made, then Create, then mentions', () => {
    const rows = rankLinks({
      query: 'Keshav',
      notes: [note("Keshav's trip"), day],
      missing: [nayyar],
      mentions: [mention('Friday, 1 May', day.id)],
      createAt,
    })
    expect(rows.map((r) => [r.kind, r.title])).toEqual([
      ['note', "Keshav's trip"],
      ['missing', 'Keshav Nayyar'],
      ['create', 'Keshav'],
      ['mention', 'Friday, 1 May'],
    ])
  })

  it('offers no Create when a note or a target already has exactly that name, or nothing is typed', () => {
    const kinds = (query: string, notes: TitleEntry[], missing = [nayyar]) => rankLinks({ query, notes, missing, createAt }).map((r) => r.kind)
    expect(kinds('keshav nayyar', [])).not.toContain('create')
    expect(kinds('Plans', [note('plans')])).not.toContain('create')
    expect(kinds('2026-05-01', [day])).not.toContain('create')
    expect(kinds('Plans', [])).toContain('create')
    expect(rankLinks({ query: '  ', notes: [], createAt })).toEqual([])
    // The note being edited counts: Create would make a second note with its name.
    const here = note('Plans')
    expect(rankLinks({ query: 'Plans', notes: [here], skip: here.id, createAt }).map((r) => r.kind)).toEqual([])
    // And where a note could not be made, there is no row.
    expect(rankLinks({ query: 'Plans', notes: [], createAt: () => null })).toEqual([])
  })

  it('shows notes that only mention the words just while fewer than five titles match, and never above them', () => {
    const five = Array.from({ length: 5 }, (_, i) => note(`Keshav ${i}`))
    const withMentions = (notes: TitleEntry[]) =>
      rankLinks({ query: 'keshav', notes, mentions: [mention('Friday, 1 May', day.id)], createAt }).map((r) => r.kind)
    expect(withMentions(five)).not.toContain('mention')
    const four = five.slice(0, 4)
    const kinds = withMentions(four)
    expect(kinds.slice(0, 4)).toEqual(['note', 'note', 'note', 'note'])
    expect(kinds.at(-1)).toBe('mention')
  })

  it('does not repeat a title match as a mention, and keeps at most four mentions', () => {
    const a = note('Keshav')
    const many = Array.from({ length: 9 }, (_, i) => mention(`Day ${i}`, `d${i}`))
    const rows = rankLinks({ query: 'keshav', notes: [a], mentions: [mention('Keshav', a.id), ...many], createAt })
    expect(rows.filter((r) => r.kind === 'mention')).toHaveLength(4)
    expect(rows.filter((r) => r.title === 'Keshav' && r.kind === 'mention')).toEqual([])
  })

  it('says how many notes link to a target', () => {
    expect(linkedFrom(1)).toBe('linked from 1 note')
    expect(linkedFrom(2)).toBe('linked from 2 notes')
  })
})

describe('large picker (#448)', () => {
  it('ranks a keystroke over 10,000 notes with stable bounded results', () => {
    const words = ['trip', 'plans', 'keshav', 'budget', 'garden', 'notes', 'ideas', 'recipe', 'reading', 'goa', 'call', 'review']
    const notes = Array.from({ length: 10_000 }, (_, i) =>
      i % 3 === 0
        ? note(`Day ${i}, ${words[i % 12]}`, `2026-${String((i % 12) + 1).padStart(2, '0')}-01T00:00:00.000Z`, `2020-01-${String((i % 28) + 1).padStart(2, '0')}`)
        : note(`${words[i % 12]} ${words[(i * 7) % 12]} ${i}`),
    )
    const missing = [{ title: 'Keshav Nayyar', path: 'Notes/Keshav Nayyar.md', count: 2 }]
    const typed = ['k', 'ke', 'kes', 'kesh', 'keshav', 'keshav n', 'keshav na', 'keshav nayyar', 'a', 'tr', 'zzzz', 'plans b']
    // Reversing the fixture must not change the best rows or grow the picker.
    for (const query of typed) {
      const input = { query, notes, missing, createAt: (s: string) => `Notes/${s}.md` }
      const rows = rankLinks(input)
      expect(rows.filter((row) => row.kind === 'note').length).toBeLessThanOrEqual(6)
      expect(rows.length).toBeLessThanOrEqual(8)
      expect(rows).toEqual(rankLinks({ ...input, notes: notes.slice().reverse() }))
    }
    expect(rankLinks({ query: 'zzzz', notes }).filter((row) => row.kind === 'note')).toEqual([])
    expect(rankLinks({ query: 'keshav nayyar', notes, missing })).toContainEqual({ kind: 'missing', ...missing[0]! })
  })
})
