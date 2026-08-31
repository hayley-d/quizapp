export function formatDuration(totalMilliseconds: number): string {
  const totalSeconds = Math.round(totalMilliseconds / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return minutes === 0 ? `${seconds}s` : `${minutes}m ${seconds}s`
}

export function formatClock(totalMilliseconds: number): string {
  const totalSeconds = Math.max(0, Math.floor(totalMilliseconds / 1000))
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  const paddedSeconds = String(seconds).padStart(2, '0')
  return hours === 0
    ? `${minutes}:${paddedSeconds}`
    : `${hours}:${String(minutes).padStart(2, '0')}:${paddedSeconds}`
}

export function formatPercentage(fraction: number | null): string {
  return fraction === null ? '—' : `${Math.round(fraction * 100)}%`
}

export function plainTextPrompt(promptMarkdown: string): string {
  return promptMarkdown
    .replace(/[#*_`~>$\\[\]()!-]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
    .slice(0, 80)
}

const MILLISECONDS_PER_MINUTE = 60_000
const MILLISECONDS_PER_HOUR = 3_600_000
const MILLISECONDS_PER_DAY = 86_400_000

export function formatRelativeTime(
  isoTimestamp: string,
  referenceMilliseconds: number = Date.now(),
): string {
  const elapsed = referenceMilliseconds - Date.parse(isoTimestamp)
  if (Number.isNaN(elapsed)) return 'unknown'
  if (elapsed < MILLISECONDS_PER_MINUTE) return 'just now'

  if (elapsed < MILLISECONDS_PER_HOUR) {
    const minutes = Math.floor(elapsed / MILLISECONDS_PER_MINUTE)
    return `${minutes} minute${minutes === 1 ? '' : 's'} ago`
  }
  if (elapsed < MILLISECONDS_PER_DAY) {
    const hours = Math.floor(elapsed / MILLISECONDS_PER_HOUR)
    return `${hours} hour${hours === 1 ? '' : 's'} ago`
  }

  const days = Math.floor(elapsed / MILLISECONDS_PER_DAY)
  if (days <= 30) return `${days} day${days === 1 ? '' : 's'} ago`

  return new Date(isoTimestamp).toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  })
}
