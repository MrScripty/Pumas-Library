import path from 'path';
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Build-time feature flag: set PUMAS_MULTI_APP=false to hide the app sidebar (non-Linux builds)
const enableMultiApp = process.env.PUMAS_MULTI_APP !== 'false';

export default defineConfig({
  // Use relative paths for assets (required for Electron file:// protocol)
  base: './',
  define: {
    __FEATURE_MULTI_APP__: JSON.stringify(enableMultiApp),
  },
  server: {
    port: 3000,
    host: '127.0.0.1',
  },
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    }
  },
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    sourcemap: false,
    minify: 'esbuild',
  }
});
