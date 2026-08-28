import { BrowserRouter, Navigate, Route, Routes } from 'react-router-dom'
import { AppShell } from '@/components/AppShell'
import { StubPage } from '@/pages/StubPage'
import { SessionPage } from '@/pages/SessionPage'
import { DecksPage } from '@/pages/DecksPage'
import { DeckPage } from '@/pages/DeckPage'
import { CardEditorPage } from '@/pages/CardEditorPage'

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<AppShell />}>
          <Route path="/" element={<Navigate to="/decks" replace />} />
          <Route path="/decks" element={<DecksPage />} />
          <Route path="/decks/:id" element={<DeckPage />} />
          <Route path="/cards/new" element={<CardEditorPage />} />
          <Route path="/cards/:id/edit" element={<CardEditorPage />} />
          <Route path="/session/:id" element={<SessionPage />} />
          <Route
            path="/stats"
            element={<StubPage title="Stats" note="Statistics arrive in part 6." />}
          />
        </Route>
      </Routes>
    </BrowserRouter>
  )
}
