import { useCallback, useEffect, useRef, useState } from 'react'

/** Which face is currently mounted. */
export type Face = 'front' | 'back'

/** One leg of the flip, in ms. The two legs make up the whole animation. */
const HALF_MS = 150

/**
 * A half-flip: rotate to edge-on, swap the face while it cannot be seen,
 * rotate back.
 *
 * Not a two-faced 3D flip. That needs both faces absolutely positioned inside
 * a fixed-height box, and a card's prompt is unclamped markdown of any height
 * (a recorded decision — see the Part 2b spec). Here only one face is ever
 * mounted, so the row keeps its natural height and the height change happens
 * during the 90° moment when the card is edge-on and invisible.
 *
 * Deliberately has no "flip started" callback. A caller that needs to react to
 * the new face watches the returned `face` in an effect — which keeps the
 * caller free of the ordering trap of passing in a callback that has to
 * reference values declared after this hook runs.
 */
export function useFlip() {
  const [face, setFace] = useState<Face>('front')
  const [angle, setAngle] = useState(0)
  // Transitions are suppressed for the single frame where the element jumps
  // from +90° to -90°: animating that would sweep it back through 0 and undo
  // the flip on screen.
  const [instant, setInstant] = useState(false)

  const busy = useRef(false)
  const cleanups = useRef<Array<() => void>>([])
  // A return-to-front requested while a flip was still in flight. The flip
  // that is running must finish — interrupting it mid-rotation would leave the
  // element at an arbitrary angle — so the request waits here and runs the
  // moment the machine settles.
  const pending = useRef<Face | null>(null)
  // `goTo` schedules a callback that may need to call `goTo` again, which it
  // cannot reference during its own definition. The ref is refreshed after
  // every render, so by the time a scheduled callback fires it holds the
  // current closure.
  const goToRef = useRef<(next: Face) => void>(() => {})

  const later = useCallback((fn: () => void, ms: number) => {
    const t = window.setTimeout(fn, ms)
    cleanups.current.push(() => window.clearTimeout(t))
  }, [])

  const nextFrame = useCallback((fn: () => void) => {
    const outer = window.requestAnimationFrame(() => {
      const inner = window.requestAnimationFrame(fn)
      cleanups.current.push(() => window.cancelAnimationFrame(inner))
    })
    cleanups.current.push(() => window.cancelAnimationFrame(outer))
  }, [])

  useEffect(
    () => () => {
      cleanups.current.forEach((c) => c())
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
        // Edge-on: swap the face and jump to the far side with no transition.
        setInstant(true)
        setFace(next)
        setAngle(-90)

        // Two frames: one to paint the -90° state, one to re-enable the
        // transition before animating home. One frame is not reliably enough.
        nextFrame(() => {
          setInstant(false)
          setAngle(0)
          later(() => {
            busy.current = false
            const queued = pending.current
            pending.current = null
            if (queued !== null) goToRef.current(queued)
          }, HALF_MS)
        })
      }, HALF_MS)
    },
    [face, later, nextFrame],
  )

  useEffect(() => {
    goToRef.current = goTo
  }, [goTo])

  const flip = useCallback(() => {
    goTo(face === 'front' ? 'back' : 'front')
  }, [face, goTo])

  /**
   * Return to the question — used when the answer fetch fails.
   *
   * Unlike `flip`, this is never dropped. The fetch it backs out is started
   * the instant `face` becomes 'back', which is the midpoint of the flip, so
   * a fast failure lands while the machine is still busy; ignoring it would
   * leave the card resting on a face whose content never loaded.
   */
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
    /** Goes on the element that rotates. */
    rotatorStyle: {
      transform: `rotateY(${angle}deg)`,
      transition: instant ? 'none' : `transform ${HALF_MS}ms ease-in-out`,
    } satisfies React.CSSProperties,
    /** Goes on the element wrapping the rotator. */
    perspectiveStyle: { perspective: '1200px' } satisfies React.CSSProperties,
  }
}
