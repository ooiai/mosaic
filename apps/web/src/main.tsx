import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import '../../../packages/ui/src/styles.css'
import App from './App.tsx'
import './index.css'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
