import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import Admin from './Admin.tsx'
import App from './App.tsx'
import Speakers from './Speakers.tsx'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    {/^\/admin(\/|$)/.test(window.location.pathname) ? (
      <Admin />
    ) : /^\/speakers\/?$/.test(window.location.pathname) ? (
      <Speakers />
    ) : (
      <App />
    )}
  </StrictMode>,
)
