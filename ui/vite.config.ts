import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

/**
 * Vite configuration for Lucy UI.
 * Proxies spec requests to the local Rust/Axum server during development.
 */
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: 'dist',
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
