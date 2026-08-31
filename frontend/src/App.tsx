import { BrowserRouter, Navigate, Route, Routes } from 'react-router-dom'
import { AppShell } from '@/components/AppShell'
import { SessionPage } from '@/pages/SessionPage'
import { MockSessionPage } from '@/pages/MockSessionPage'
import { DecksPage } from '@/pages/DecksPage'
import { DeckPage } from '@/pages/DeckPage'
import { CardEditorPage } from '@/pages/CardEditorPage'
import { FlashcardsPage } from '@/pages/FlashcardsPage'

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
          <Route path="/mock/:id" element={<MockSessionPage />} />
          <Route path="/flashcards/:id" element={<FlashcardsPage />} />
        </Route>
      </Routes>
    </BrowserRouter>
  )
}
