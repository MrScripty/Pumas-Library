import path from 'path';
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { devServerCspPlugin } from './src/config/devServerCsp';

const DEV_SERVER_HOST = '127.0.0.1';
const DEV_SERVER_PORT = 3000;

export default defineConfig(({ mode }) => {
  // The explicit mode keeps the npm script cross-platform; the environment
  // variable lets the shared desktop launcher select the same build.
  const enableInferencePlugins =
    mode !== 'library-only' && process.env.PUMAS_INFERENCE_PLUGINS !== 'false';

  return {
    // Use relative paths for assets (required for Electron file:// protocol)
    base: './',
    define: {
      __FEATURE_INFERENCE_PLUGINS__: JSON.stringify(enableInferencePlugins),
    },
    server: {
      port: DEV_SERVER_PORT,
      host: DEV_SERVER_HOST,
    },
    // The production CSP has no loopback connect-src; only `vite serve` adds
    // its HMR WebSocket.
    plugins: [react(), devServerCspPlugin(DEV_SERVER_HOST, DEV_SERVER_PORT)],
    resolve: {
      alias: {
        '@app-entry': path.resolve(
          __dirname,
          enableInferencePlugins ? './src/App.tsx' : './src/components/LibraryOnlyApp.tsx'
        ),
        '@runtime-route-editor': path.resolve(
          __dirname,
          enableInferencePlugins
            ? './src/components/ModelRuntimeRouteEditor.tsx'
            : './src/components/LibraryOnlyRuntimeRouteEditor.tsx'
        ),
        '@runtime-model-serve-action': path.resolve(
          __dirname,
          enableInferencePlugins
            ? './src/components/RuntimeModelServeAction.tsx'
            : './src/components/LibraryOnlyRuntimeModelServeAction.tsx'
        ),
        '@': path.resolve(__dirname, './src'),
      },
    },
    build: {
      outDir: 'dist',
      assetsDir: 'assets',
      sourcemap: false,
      minify: 'esbuild',
      rollupOptions: {
        output: {
          manualChunks(id) {
            if (id.includes('node_modules')) {
              return 'vendor';
            }

            return undefined;
          },
        },
      },
    },
  };
});
