export function StubPage({ title, note }: { title: string; note: string }) {
  return (
    <div>
      <h1 className="font-display text-2xl font-bold">{title}</h1>
      <p className="mt-2 text-muted-foreground">{note}</p>
    </div>
  )
}
