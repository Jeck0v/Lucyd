import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

/**
 * Vite configuration for Lucy UI.
 * Proxies spec requests to the local Rust/Axum server during development.
 */
export default defineConfig({
  plugins: [react()],

  // Base public path — all asset references in the built index.html will be
  // prefixed with /docs/ so the browser fetches them from the right location
  // when the app is served at http://host/docs/ by lucy-core.
  base: '/docs/',

  build: {
    outDir: '../crates/lucy-core/ui/dist',
    emptyOutDir: true,
  },
  server: {
    proxy: {
      '/docs/spec.json': {
        target: 'http://localhost:3000',
        changeOrigin: true,
      },
    },
  },
})
