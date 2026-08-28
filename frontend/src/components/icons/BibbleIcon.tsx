type BibbleIconProps = {
  className?: string
}

export function BibbleIcon({ className }: BibbleIconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.6}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={className}
    >
      <path d="M18.2 12A2.4 2.4 0 0 1 16.38 16.38 2.4 2.4 0 0 1 12 18.2 2.4 2.4 0 0 1 7.62 16.38 2.4 2.4 0 0 1 5.8 12 2.4 2.4 0 0 1 7.62 7.62 2.4 2.4 0 0 1 12 5.8 2.4 2.4 0 0 1 16.38 7.62 2.4 2.4 0 0 1 18.2 12Z" />
      <path d="M10 12.6h.01" />
      <path d="M14 12.6h.01" />
    </svg>
  )
}
