import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Tauri/Rust writes hundreds of files under this directory while compiling.
      // Vite must not watch it, otherwise Windows can raise EBUSY on locked .exe files.
      ignored: ['**/src-tauri/**']
    }
  },
  envPrefix: ['VITE_']
});
