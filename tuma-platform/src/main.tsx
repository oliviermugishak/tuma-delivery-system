import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from '@tanstack/react-router'

import { getRouter } from './router'

import './styles.css'

// The platform is a client-only SPA: every page sits behind auth, so SSR has
// nothing to offer and the session (httpOnly cookies) only resolves here.
createRoot(document.getElementById('app')!).render(
  <StrictMode>
    <RouterProvider router={getRouter()} />
  </StrictMode>,
)
