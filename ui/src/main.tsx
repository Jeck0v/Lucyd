import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'

/**
 * Application entry point.
 * Mounts the React tree into the `#root` element defined in `index.html`.
 */
ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
