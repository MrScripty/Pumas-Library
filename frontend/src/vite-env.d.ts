/// <reference types="vite/client" />
/// <reference types="@testing-library/jest-dom/vitest" />

interface ImportMetaEnv {
  readonly MODE: string;
  readonly BASE_URL: string;
  readonly PROD: boolean;
  readonly DEV: boolean;
  readonly SSR: boolean;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

/** Build-time feature flag: multi-app sidebar enabled (Linux only) */
declare const __FEATURE_MULTI_APP__: boolean;
