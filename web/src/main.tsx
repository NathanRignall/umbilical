import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import Admin from './Admin.tsx'
import App from './App.tsx'
import Access from './Access.tsx'

// /admin is the host's page; /<group> (e.g. /speakers, /tv) is a QR code; anything else,
// notably UniFi's /guest/s/<site>/ redirect, is the portal.
function Root() {
  const path = window.location.pathname
  if (/^\/admin(\/|$)/.test(path)) return <Admin />
  const group = path.match(/^\/([a-z0-9-]+)\/?$/)?.[1]
  if (group && group !== 'guest') return <Access group={group} />
  return <App />
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Root />
  </StrictMode>,
)
