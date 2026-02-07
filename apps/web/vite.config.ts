import react from '@vitejs/plugin-react'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vite'

const appRoot = fileURLToPath(new URL('.', import.meta.url))

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@mosaic/ui': path.resolve(appRoot, '../../packages/ui/src/index.ts'),
      '@mosaic/workbench': path.resolve(appRoot, '../../packages/workbench/src/index.ts'),
      react: path.resolve(appRoot, 'node_modules/react'),
      'react/jsx-runtime': path.resolve(appRoot, 'node_modules/react/jsx-runtime.js'),
      'react/jsx-dev-runtime': path.resolve(appRoot, 'node_modules/react/jsx-dev-runtime.js'),
      '@tauri-apps/api/core': path.resolve(appRoot, 'node_modules/@tauri-apps/api/core.js'),
    },
  },
})
