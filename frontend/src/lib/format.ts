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
