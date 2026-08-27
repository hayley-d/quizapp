import { NavLink, Outlet } from 'react-router-dom'
import { Toaster } from '@/components/ui/sonner'

const navigationLinks = [
  { to: '/study', label: 'Study' },
  { to: '/decks', label: 'Decks' },
  { to: '/stats', label: 'Stats' },
]

export function AppShell() {
  return (
    <div className="min-h-dvh">
      <header className="border-b">
        <nav className="mx-auto flex max-w-5xl items-center gap-1 px-4 py-3">
          <span className="font-display mr-4 text-lg font-bold text-primary">quizapp</span>
          {navigationLinks.map((navigationLink) => (
            <NavLink
              key={navigationLink.to}
              to={navigationLink.to}
              className={({ isActive }) =>
                `rounded-lg px-3 py-1.5 text-sm font-medium transition-colors ${
                  isActive
                    ? 'bg-primary text-primary-foreground'
                    : 'text-muted-foreground hover:bg-secondary'
                }`
              }
            >
              {navigationLink.label}
            </NavLink>
          ))}
        </nav>
      </header>
      <main className="mx-auto max-w-5xl px-4 py-8">
        <Outlet />
      </main>
      <Toaster />
    </div>
  )
}
