import path from 'path';
import { configDefaults, defineConfig } from 'vitest/config';
import base from './vitest.config';

// Library-only renderer variant. Mirrors `vite build --mode library-only`:
// inference plugins are compiled out and the plugin entry points resolve to
// their library-only replacements. The default run cannot cover this because
// the feature flag is a compile-time define.
export default defineConfig({
  ...base,
  define: {
    __FEATURE_INFERENCE_PLUGINS__: false,
  },
  test: {
    ...base.test,
    include: ['src/**/*.library-only.test.{ts,tsx}'],
    exclude: configDefaults.exclude,
  },
  resolve: {
    alias: {
      '@app-entry': path.resolve(__dirname, './src/components/LibraryOnlyApp.tsx'),
      '@runtime-route-editor': path.resolve(__dirname, './src/components/LibraryOnlyRuntimeRouteEditor.tsx'),
      '@runtime-model-serve-action': path.resolve(
        __dirname,
        './src/components/LibraryOnlyRuntimeModelServeAction.tsx'
      ),
      '@': path.resolve(__dirname, './src'),
    },
  },
});
