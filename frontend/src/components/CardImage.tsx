import { useState } from 'react'
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog'
import { cn } from '@/lib/utils'

type CardImageProps = {
  path: string
  altText: string
  className?: string
}

export function CardImage({ path, altText, className }: CardImageProps) {
  const [isEnlarged, setIsEnlarged] = useState(false)
  const imageUrl = `/${path}`

  return (
    <>
      <button
        type="button"
        onClick={() => setIsEnlarged(true)}
        aria-label={`Enlarge image: ${altText}`}
        title="Enlarge image"
        className={cn(
          'shrink-0 overflow-hidden rounded-md border bg-muted/30',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
          className,
        )}
      >
        <img src={imageUrl} alt={altText} className="h-16 w-auto max-w-32 object-contain" />
      </button>

      <Dialog open={isEnlarged} onOpenChange={setIsEnlarged}>
        <DialogContent className="sm:max-w-3xl" aria-describedby={undefined}>
          <DialogTitle className="sr-only">{altText}</DialogTitle>
          <img src={imageUrl} alt={altText} className="max-h-[75vh] w-full object-contain" />
        </DialogContent>
      </Dialog>
    </>
  )
}
