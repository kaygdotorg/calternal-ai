/**
 * How Calternal writes a day for people: "Wednesday, 7 October", with the
 * year only when it isn't the current one ("Monday, 6 October 2025").
 * Every place that shows a date uses this, so they all read the same.
 */

export function formatDay(date: Date, now: Date = new Date(), locale = 'en-GB'): string {
  const part = (opts: Intl.DateTimeFormatOptions) => new Intl.DateTimeFormat(locale, opts).format(date)
  const day = `${part({ weekday: 'long' })}, ${part({ day: 'numeric' })} ${part({ month: 'long' })}`
  return date.getFullYear() === now.getFullYear() ? day : `${day} ${date.getFullYear()}`
}
