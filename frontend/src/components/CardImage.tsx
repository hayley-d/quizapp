import { useState } from 'react'
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog'
import { cn } from '@/lib/utils'

type Props = {
  /** `image_path` exactly as stored on the card: `images/<hash>.<ext>`. */
  path: string
  /** Alt text. Pass the prompt, so a screen reader gets the question rather than "image". */
  alt: string
  className?: string
}

/**
 * A card's image: a small thumbnail that opens full-size in a dialog.
 *
 * The thumbnail is the same file scaled by CSS. There is no resizing pipeline
 * and, for hand-cropped diagrams pulled out of lecture slides, there does not
 * need to be — the whole file is tens of kilobytes.
 *
 * `image_path` is stored relative to the data directory and the server serves
 * that directory at `/images`, so the URL is the path with a leading slash.
 * That is the only place the two halves meet; keep it here rather than
 * building URLs at each call site.
 */
export function CardImage({ path, alt, className }: Props) {
  const [open, setOpen] = useState(false)
  const src = `/${path}`

  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        aria-label={`Enlarge image: ${alt}`}
        title="Enlarge image"
        className={cn(
          'shrink-0 overflow-hidden rounded-md border bg-muted/30',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
          className,
        )}
      >
        <img src={src} alt={alt} className="h-16 w-auto max-w-32 object-contain" />
      </button>

      <Dialog open={open} onOpenChange={setOpen}>
        {/* aria-describedby is cleared because there is no description to
            point at — Radix warns about a missing one otherwise. */}
        <DialogContent className="sm:max-w-3xl" aria-describedby={undefined}>
          <DialogTitle className="sr-only">{alt}</DialogTitle>
          <img src={src} alt={alt} className="max-h-[75vh] w-full object-contain" />
        </DialogContent>
      </Dialog>
    </>
  )
}
