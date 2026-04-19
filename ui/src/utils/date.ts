/** Format a UTC ISO string or epoch-ms as locale date (e.g. "3/27/2026") */
export function formatDate(value: string | number): string {
  return new Date(value).toLocaleDateString()
}

/** Format a UTC ISO string or epoch-ms as locale date + time (e.g. "3/27/2026 · 14:30") */
export function formatDateTime(value: string | number): string {
  const d = new Date(value)
  return `${d.toLocaleDateString()} · ${d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`
}

/** Format a UTC ISO string or epoch-ms as locale time (e.g. "14:30") */
export function formatTime(value: string | number): string {
  return new Date(value).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

/**
 * Format a plain YYYY-MM-DD date string as a locale date.
 * Parsed as local midnight (not UTC) since air dates are timezone-naive.
 */
export function formatAirDate(dateStr: string): string {
  const [year, month, day] = dateStr.split('-').map(Number)
  return new Date(year, month - 1, day).toLocaleDateString(undefined, {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  })
}
