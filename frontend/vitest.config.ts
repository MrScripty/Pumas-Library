import { configDefaults, defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  define: {
    __FEATURE_INFERENCE_PLUGINS__: true,
  },
  plugins: [react()],
  test: {
    // Library-only tests need the compiled-out feature flag; see vitest.library-only.config.ts.
    exclude: [...configDefaults.exclude, 'conformance/**', 'src/**/*.library-only.test.{ts,tsx}'],
    globals: true,
    environment: 'jsdom',
    setupFiles: './src/test/setup.ts',
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      exclude: [
        'node_modules/',
        'src/test/',
        '**/*.d.ts',
        '**/*.config.*',
        '**/mockData/',
        'dist/',
      ],
    },
  },
  resolve: {
    alias: {
      '@app-entry': path.resolve(__dirname, './src/App.tsx'),
      '@runtime-route-editor': path.resolve(__dirname, './src/components/ModelRuntimeRouteEditor.tsx'),
      '@runtime-model-serve-action': path.resolve(__dirname, './src/components/RuntimeModelServeAction.tsx'),
      '@': path.resolve(__dirname, './src'),
    },
  },
});
