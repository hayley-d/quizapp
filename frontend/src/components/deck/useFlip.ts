import { useCallback, useEffect, useRef, useState } from 'react'

export type Face = 'front' | 'back'

const HALF_FLIP_MS = 150

export function useFlip() {
  const [face, setFace] = useState<Face>('front')
  const [angle, setAngle] = useState(0)
  const [instant, setInstant] = useState(false)

  const busy = useRef(false)
  const cleanups = useRef<Array<() => void>>([])
  const pending = useRef<Face | null>(null)
  const latestGoTo = useRef<(next: Face) => void>(() => {})

  const later = useCallback((callback: () => void, delayMs: number) => {
    const timeoutId = window.setTimeout(callback, delayMs)
    cleanups.current.push(() => window.clearTimeout(timeoutId))
  }, [])

  const nextFrame = useCallback((callback: () => void) => {
    const outerFrameId = window.requestAnimationFrame(() => {
      const innerFrameId = window.requestAnimationFrame(callback)
      cleanups.current.push(() => window.cancelAnimationFrame(innerFrameId))
    })
    cleanups.current.push(() => window.cancelAnimationFrame(outerFrameId))
  }, [])

  useEffect(
    () => () => {
      cleanups.current.forEach((cleanup) => cleanup())
      cleanups.current = []
    },
    [],
  )

  const goTo = useCallback(
    (next: Face) => {
      if (busy.current || next === face) return

      if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
        setFace(next)
        return
      }

      busy.current = true
      setInstant(false)
      setAngle(90)

      later(() => {
        setInstant(true)
        setFace(next)
        setAngle(-90)

        nextFrame(() => {
          setInstant(false)
          setAngle(0)
          later(() => {
            busy.current = false
            const queued = pending.current
            pending.current = null
            if (queued !== null) latestGoTo.current(queued)
          }, HALF_FLIP_MS)
        })
      }, HALF_FLIP_MS)
    },
    [face, later, nextFrame],
  )

  useEffect(() => {
    latestGoTo.current = goTo
  }, [goTo])

  const flip = useCallback(() => {
    goTo(face === 'front' ? 'back' : 'front')
  }, [face, goTo])

  const toFront = useCallback(() => {
    if (busy.current) {
      pending.current = 'front'
      return
    }
    goTo('front')
  }, [goTo])

  return {
    face,
    flip,
    toFront,
    rotatorStyle: {
      transform: `rotateY(${angle}deg)`,
      transition: instant ? 'none' : `transform ${HALF_FLIP_MS}ms ease-in-out`,
    } satisfies React.CSSProperties,
    perspectiveStyle: { perspective: '1200px' } satisfies React.CSSProperties,
  }
}
