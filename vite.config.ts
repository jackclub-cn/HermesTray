import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  // Hide dev window until toggled
  server: {
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/target/**', '**/node_modules/**'],
    },
  },
})
