import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'node:path'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { '@': path.resolve(import.meta.dirname, './src') } },
  server: {
    port: 5273,
    proxy: {
      '/api': { target: 'http://127.0.0.1:3000', changeOrigin: true },
      // Uploaded images are served by the API, not by Vite. Without this the
      // dev server answers /images/... with index.html and every thumbnail is
      // a broken image.
      '/images': { target: 'http://127.0.0.1:3000', changeOrigin: true },
    },
  },
})
