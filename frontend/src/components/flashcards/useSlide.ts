import { useCallback, useEffect, useRef, useState } from 'react'
import { usePrefersReducedMotion } from '@/hooks/usePrefersReducedMotion'

export type SlideDirection = 'next' | 'previous'

const HALF_SLIDE_MS = 130
const SLIDE_DISTANCE_PX = 56

export function useSlide() {
  const [offsetPixels, setOffsetPixels] = useState(0)
  const [faded, setFaded] = useState(false)
  const [instant, setInstant] = useState(false)
  const prefersReducedMotion = usePrefersReducedMotion()

  const busy = useRef(false)
  const cleanups = useRef<Array<() => void>>([])

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

  const slide = useCallback(
    (direction: SlideDirection, swapContent: () => void) => {
      if (prefersReducedMotion || busy.current) {
        swapContent()
        return
      }

      const departure = direction === 'next' ? -SLIDE_DISTANCE_PX : SLIDE_DISTANCE_PX

      busy.current = true
      setInstant(false)
      setOffsetPixels(departure)
      setFaded(true)

      later(() => {
        setInstant(true)
        setOffsetPixels(-departure)
        swapContent()

        nextFrame(() => {
          setInstant(false)
          setOffsetPixels(0)
          setFaded(false)
          later(() => {
            busy.current = false
          }, HALF_SLIDE_MS)
        })
      }, HALF_SLIDE_MS)
    },
    [later, nextFrame, prefersReducedMotion],
  )

  return {
    slide,
    sliderStyle: {
      transform: `translateX(${offsetPixels}px)`,
      opacity: faded ? 0 : 1,
      transition: instant
        ? 'none'
        : `transform ${HALF_SLIDE_MS}ms ease-in-out, opacity ${HALF_SLIDE_MS}ms ease-in-out`,
    } satisfies React.CSSProperties,
  }
}
