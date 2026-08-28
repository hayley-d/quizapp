type MakkaPakkaIconProps = {
  className?: string
}

export function MakkaPakkaIcon({ className }: MakkaPakkaIconProps) {
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
      <ellipse cx="12" cy="5" rx="3.8" ry="2" />
      <ellipse cx="12" cy="12" rx="5.4" ry="2" />
      <ellipse cx="12" cy="19" rx="7" ry="2" />
    </svg>
  )
}
