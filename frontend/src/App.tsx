import { BrowserRouter, Navigate, Route, Routes } from 'react-router-dom'
import { AppShell } from '@/components/AppShell'
import { StubPage } from '@/pages/StubPage'

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<AppShell />}>
          <Route path="/" element={<Navigate to="/decks" replace />} />
          <Route
            path="/decks"
            element={<StubPage title="Decks" note="Deck management arrives in Task 7." />}
          />
          <Route
            path="/study"
            element={<StubPage title="Study" note="Session modes arrive in part 3." />}
          />
          <Route
            path="/stats"
            element={<StubPage title="Stats" note="Statistics arrive in part 6." />}
          />
        </Route>
      </Routes>
    </BrowserRouter>
  )
}
