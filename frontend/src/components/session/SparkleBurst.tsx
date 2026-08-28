import type { CSSProperties } from 'react'

const SPARKLE_COUNT = 8
const BURST_RADIUS_PX = 46

type SparkleStyle = CSSProperties & {
  '--sparkle-x': string
  '--sparkle-y': string
}

const SPARKLES: SparkleStyle[] = Array.from({ length: SPARKLE_COUNT }, (_, sparkleIndex) => {
  const angleRadians = (sparkleIndex / SPARKLE_COUNT) * Math.PI * 2
  const distancePx = BURST_RADIUS_PX * (sparkleIndex % 2 === 0 ? 1 : 0.65)
  return {
    '--sparkle-x': `${Math.round(Math.cos(angleRadians) * distancePx)}px`,
    '--sparkle-y': `${Math.round(Math.sin(angleRadians) * distancePx)}px`,
    animationDelay: `${sparkleIndex * 25}ms`,
  }
})

export function SparkleBurst() {
  return (
    <div
      aria-hidden="true"
      className="pointer-events-none absolute inset-0 overflow-visible"
    >
      {SPARKLES.map((sparkleStyle, sparkleIndex) => (
        <span
          key={sparkleIndex}
          style={sparkleStyle}
          className="sparkle absolute top-1/2 left-8 block size-2 rounded-full bg-sparkle"
        />
      ))}
    </div>
  )
}
