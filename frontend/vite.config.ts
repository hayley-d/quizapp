import { defineConfig, type Plugin } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'node:path'

// Rewrites rather than prunes dist, so a KaTeX or Fontsource upgrade cannot silently
// reintroduce the formats. The leading comma in the pattern is load-bearing: it means
// a face whose only src is woff keeps it, so this can never leave a face with no src.
function woff2OnlyFontFaces(): Plugin {
  const trailingLegacyFormat =
    /,\s*url\([^)]+\)\s*format\((['"])(?:woff|truetype|opentype)\1\)/g

  return {
    name: 'woff2-only-font-faces',
    enforce: 'pre',
    transform(code, id) {
      if (!id.includes('.css') || !code.includes('@font-face')) return null
      const rewritten = code.replace(trailingLegacyFormat, '')
      return rewritten === code ? null : { code: rewritten, map: null }
    },
  }
}

export default defineConfig({
  plugins: [react(), tailwindcss(), woff2OnlyFontFaces()],
  resolve: { alias: { '@': path.resolve(import.meta.dirname, './src') } },
  build: {
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            // Higher priority is load-bearing: includeDependenciesRecursively
            // defaults to true, so without capturing React first the markdown group
            // swallows it as a dependency of react-markdown, leaving every other
            // chunk depending on markdown just to reach React.
            {
              name: 'react',
              priority: 20,
              test: /[\\/]node_modules[\\/](react|react-dom|scheduler)[\\/]/,
            },
            {
              name: 'markdown',
              priority: 10,
              test: /[\\/]node_modules[\\/](katex|react-markdown|remark-math|rehype-katex)[\\/]/,
            },
            {
              name: 'dnd',
              priority: 10,
              test: /[\\/]node_modules[\\/]@dnd-kit[\\/]/,
            },
          ],
        },
      },
    },
  },
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
