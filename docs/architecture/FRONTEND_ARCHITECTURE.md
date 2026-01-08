# Frontend Refactoring Plan

**Status:** Planning
**Created:** 2026-01-07
**Goal:** Align frontend code quality with backend standards

---

## Overview

This document outlines the plan to refactor the frontend codebase to match the strict coding standards enforced in the backend. The main issues are:

1. **Monolithic components** - App.tsx is 1024 lines (target: <300 lines per file)
2. **Weak type safety** - 16 instances of `any` type in App.tsx alone
3. **Inconsistent error handling** - Generic catch blocks, no custom error types
4. **Missing enforcement** - No pre-commit hooks, lax TypeScript/ESLint rules
5. **Inconsistent logging** - Logger utility exists but not consistently used

---

## Phase 1: Establish Frontend Coding Standards

### 1.1 Create Error Hierarchy

**File:** `frontend/src/errors/index.ts`

Create typed error classes matching backend pattern from `backend/exceptions.py`:

```typescript
export class ComfyUILauncherError extends Error {
  constructor(message: string, public cause?: Error) {
    super(message);
    this.name = this.constructor.name;
    if (Error.captureStackTrace) {
      Error.captureStackTrace(this, this.constructor);
    }
  }
}

export class NetworkError extends ComfyUILauncherError {
  constructor(
    message: string,
    public url?: string,
    public status?: number,
    cause?: Error
  ) {
    super(message, cause);
  }
}

export class APIError extends ComfyUILauncherError {
  constructor(
    message: string,
    public endpoint?: string,
    cause?: Error
  ) {
    super(message, cause);
  }
}

export class ValidationError extends ComfyUILauncherError {
  constructor(
    message: string,
    public field?: string,
    cause?: Error
  ) {
    super(message, cause);
  }
}

export class MetadataError extends ComfyUILauncherError {
  constructor(
    message: string,
    public filePath?: string,
    cause?: Error
  ) {
    super(message, cause);
  }
}

export class ProcessError extends ComfyUILauncherError {
  constructor(
    message: string,
    public exitCode?: number,
    cause?: Error
  ) {
    super(message, cause);
  }
}

export class ResourceError extends ComfyUILauncherError {
  constructor(
    message: string,
    public resourceType?: string,
    cause?: Error
  ) {
    super(message, cause);
  }
}

// Type guard helper
export function isKnownError(error: unknown): error is ComfyUILauncherError {
  return error instanceof ComfyUILauncherError;
}
```

**Success Criteria:**
- [ ] Error hierarchy created
- [ ] All error classes have proper typing
- [ ] Type guard helper implemented

---

### 1.2 Strengthen TypeScript Configuration

**File:** `frontend/tsconfig.json`

**Current State:** Lax compiler options allowing `any` types

**Changes:**
```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "skipLibCheck": true,
    "types": ["node"],

    // Module resolution
    "moduleResolution": "bundler",
    "isolatedModules": true,
    "moduleDetection": "force",
    "allowJs": true,
    "jsx": "react-jsx",
    "paths": {
      "@/*": ["./*"]
    },
    "allowImportingTsExtensions": true,
    "noEmit": true,

    // ✅ NEW: Strict type checking
    "strict": true,
    "noImplicitAny": true,
    "strictNullChecks": true,
    "strictFunctionTypes": true,
    "strictBindCallApply": true,
    "strictPropertyInitialization": true,
    "noImplicitThis": true,
    "alwaysStrict": true,

    // ✅ NEW: Additional checks
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noImplicitReturns": true,
    "noFallthroughCasesInSwitch": true,
    "noUncheckedIndexedAccess": true,
    "noImplicitOverride": true,
    "noPropertyAccessFromIndexSignature": true
  }
}
```

**Success Criteria:**
- [ ] TypeScript strict mode enabled
- [ ] All strict flags configured
- [ ] Build passes with new configuration

---

### 1.3 Enhance ESLint Rules

**File:** `frontend/eslint.config.js`

**Changes:**
```javascript
import js from '@eslint/js';
import react from 'eslint-plugin-react';
import jsxA11y from 'eslint-plugin-jsx-a11y';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  js.configs.recommended,
  ...tseslint.configs.strictTypeChecked,  // ✅ NEW: Strict type checking
  react.configs.flat.recommended,
  jsxA11y.flatConfigs.recommended,
  {
    ignores: ['dist/**', 'node_modules/**', '*.config.js', '*.config.ts'],
  },
  {
    files: ['src/**/*.{ts,tsx}'],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      parserOptions: {
        ecmaFeatures: { jsx: true },
        project: './tsconfig.json',  // ✅ NEW: Enable type-aware linting
      },
    },
    settings: {
      react: { version: 'detect' },
    },
    rules: {
      // ✅ NEW: Prevent console usage (must use logger)
      'no-console': ['error', { allow: [] }],

      // ✅ NEW: Enforce proper error handling
      '@typescript-eslint/no-floating-promises': 'error',
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/explicit-function-return-type': ['warn', {
        allowExpressions: true,
        allowTypedFunctionExpressions: true,
      }],
      '@typescript-eslint/no-unused-vars': ['error', {
        argsIgnorePattern: '^_',
        varsIgnorePattern: '^_',
      }],

      // ✅ NEW: File size and complexity limits
      'max-lines': ['warn', {
        max: 300,
        skipBlankLines: true,
        skipComments: true
      }],
      'max-lines-per-function': ['warn', {
        max: 50,
        skipBlankLines: true,
        skipComments: true,
      }],
      'complexity': ['warn', 15],

      // ✅ NEW: Prevent generic Error usage
      'no-restricted-syntax': [
        'error',
        {
          selector: 'ThrowStatement > NewExpression[callee.name="Error"]',
          message: 'Use specific error types from @/errors instead of generic Error',
        },
        {
          selector: 'CatchClause > Identifier[name="error"]:not([typeAnnotation])',
          message: 'Catch clauses should use type guards (if (error instanceof ...))',
        },
        // Existing React Aria rules...
        {
          selector: 'JSXAttribute[name.name="onMouseEnter"]',
          message: 'Avoid using onMouseEnter. Use React Aria\'s useHover hook.',
        },
        {
          selector: 'JSXAttribute[name.name="onMouseLeave"]',
          message: 'Avoid using onMouseLeave. Use React Aria\'s useHover hook.',
        },
        {
          selector: 'JSXAttribute[name.name="onMouseOver"]',
          message: 'Avoid using onMouseOver. Use React Aria\'s useHover hook.',
        },
        {
          selector: 'JSXAttribute[name.name="onMouseOut"]',
          message: 'Avoid using onMouseOut. Use React Aria\'s useHover hook.',
        },
      ],

      // Existing accessibility rules
      'jsx-a11y/mouse-events-have-key-events': 'error',
      'jsx-a11y/no-static-element-interactions': 'warn',
    },
  }
);
```

**Success Criteria:**
- [ ] ESLint rules updated
- [ ] Type-aware linting enabled
- [ ] Lint passes with new rules (after fixes)

---

### 1.4 Create Pre-commit Hook Scripts

**File:** `frontend/scripts/check-error-handling.js`

Create automated checks for error handling violations:

```javascript
#!/usr/bin/env node
import { readFileSync } from 'fs';
import { globSync } from 'glob';

const violations = [];
const files = globSync('src/**/*.{ts,tsx}');

files.forEach((file) => {
  const content = readFileSync(file, 'utf-8');
  const lines = content.split('\n');

  lines.forEach((line, index) => {
    const lineNum = index + 1;

    // Check for console.log/error (unless noqa comment)
    if (/console\.(log|error|warn|info|debug)/.test(line) && !/noqa:.*console/.test(line)) {
      violations.push(`${file}:${lineNum} - Use logger instead of console.*`);
    }

    // Check for catch without type checking
    if (/catch\s*\(\s*\w+\s*\)\s*{/.test(line)) {
      const nextFewLines = lines.slice(index, index + 10).join('\n');
      const hasTypeGuard = /instanceof/.test(nextFewLines);
      const hasNoqa = /noqa:.*generic-exception/.test(line);

      if (!hasTypeGuard && !hasNoqa) {
        violations.push(
          `${file}:${lineNum} - Catch block must use type guards (instanceof) for specific error handling`
        );
      }
    }

    // Check for throw new Error()
    if (/throw new Error\(/.test(line) && !/noqa:.*generic-error/.test(line)) {
      violations.push(
        `${file}:${lineNum} - Use specific error types from @/errors instead of generic Error`
      );
    }
  });
});

if (violations.length > 0) {
  console.error('\n❌ Error Handling Violations Found:\n');
  violations.forEach((v) => console.error(`  ${v}`));
  console.error('\nAdd // noqa: generic-exception or fix the error handling\n');
  process.exit(1);
}

console.log('✅ Error handling checks passed');
```

**File:** `frontend/scripts/check-file-size.js`

```javascript
#!/usr/bin/env node
import { readFileSync } from 'fs';
import { globSync } from 'glob';

const MAX_LINES = 300;
const violations = [];
const files = globSync('src/**/*.{ts,tsx}', {
  ignore: ['**/*.test.{ts,tsx}', '**/*.backup.*']
});

files.forEach((file) => {
  const content = readFileSync(file, 'utf-8');
  const lines = content.split('\n').filter(line => {
    const trimmed = line.trim();
    return trimmed !== '' && !trimmed.startsWith('//');
  });

  if (lines.length > MAX_LINES) {
    violations.push(`${file}: ${lines.length} lines (max ${MAX_LINES})`);
  }
});

if (violations.length > 0) {
  console.error(`\n❌ File Size Violations Found (>${MAX_LINES} lines):\n`);
  violations.forEach((v) => console.error(`  ${v}`));
  console.error('\nRefactor large files into smaller modules\n');
  process.exit(1);
}

console.log(`✅ File size checks passed (all files <${MAX_LINES} lines)`);
```

**Update:** `frontend/package.json`

```json
{
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview",
    "lint": "eslint . --ext ts,tsx --report-unused-disable-directives --max-warnings 0",
    "lint:fix": "eslint . --ext ts,tsx --fix",
    "test": "vitest",
    "test:ui": "vitest --ui",
    "test:run": "vitest run",
    "test:coverage": "vitest run --coverage",
    "check:errors": "node scripts/check-error-handling.js",
    "check:size": "node scripts/check-file-size.js",
    "check:types": "tsc --noEmit",
    "precommit": "npm run lint && npm run check:errors && npm run check:size && npm run check:types"
  }
}
```

**Success Criteria:**
- [ ] Error handling check script created
- [ ] File size check script created
- [ ] Scripts integrated into package.json
- [ ] Scripts executable and passing

---

### 1.5 Create Frontend Contributing Guidelines

**File:** `frontend/CONTRIBUTING.md`

```markdown
# Frontend Contributing Guidelines

This document outlines the coding standards for the ComfyUI Launcher frontend. These standards mirror the backend requirements to ensure consistency across the codebase.

## Table of Contents
- [Type Safety](#type-safety)
- [Error Handling](#error-handling)
- [Logging](#logging)
- [Component Size](#component-size)
- [File Organization](#file-organization)
- [Pre-commit Checks](#pre-commit-checks)

---

## Type Safety

**DO NOT use `any` type.** Always define proper interfaces and types.

```typescript
// ❌ BAD - Using any
function processData(data: any): any {
  return data.value;
}

// ✅ GOOD - Proper typing
interface DataResponse {
  value: string;
  timestamp: number;
}

function processData(data: DataResponse): string {
  return data.value;
}
```

**Enable strict TypeScript:**
- All strict compiler options are enabled in `tsconfig.json`
- ESLint enforces `@typescript-eslint/no-explicit-any`
- Pre-commit hooks check for type safety

**Exception:** Use `// @ts-expect-error: <reason>` sparingly for legitimate cases.

---

## Error Handling

**DO NOT use generic exception handlers.** Always use type guards to check specific error types.

**Every error handler must log** using `logger.*` with the appropriate level.
**Exception:** If the handler only re-throws and performs no other work, logging may be omitted to avoid double logging.

```typescript
import { getLogger } from '@/utils/logger';
import { NetworkError, APIError, ValidationError } from '@/errors';

const logger = getLogger('ComponentName');

// ❌ BAD - Generic catch
try {
  await fetchData();
} catch (error) {
  console.error('Error:', error);  // Also bad - use logger
}

// ❌ BAD - Multiple types in one check
try {
  await fetchData();
} catch (error) {
  if (error instanceof NetworkError || error instanceof APIError) {
    logger.error('Failed', error);
  }
}

// ✅ GOOD - Specific error handling
try {
  await fetchData();
} catch (error) {
  if (error instanceof NetworkError) {
    logger.error('Network request failed', { url: error.url, status: error.status });
    throw new APIError(`Failed to fetch: ${error.message}`, 'fetchData', error);
  }
  if (error instanceof ValidationError) {
    logger.error('Validation failed', { field: error.field });
    setErrorMessage(error.message);
    return;
  }
  // Unknown error - always log and re-throw
  logger.error('Unexpected error', { error });
  throw error;
}

// ✅ GOOD - Raise-only handler (logging optional)
try {
  await operation();
} catch (error) {
  if (error instanceof ValidationError) {
    // No logging needed - just transform and re-throw
    throw new APIError(`Validation failed: ${error.message}`, undefined, error);
  }
  throw error;
}
```

**Custom Exceptions:**
All custom exceptions are defined in `frontend/src/errors/index.ts`:
- `ComfyUILauncherError` - Base exception
- `NetworkError` - Network operations
- `APIError` - PyWebView API failures
- `ValidationError` - Input validation failures
- `MetadataError` - Metadata corruption
- `ProcessError` - Process management failures
- `ResourceError` - Resource management failures

**Pre-commit Hook:** Automatically detects:
- `console.log/error/warn` usage (use logger instead)
- Catch blocks without type guards (`instanceof`)
- `throw new Error()` (use specific error types)

**Exception:** Use `// noqa: generic-exception` for cases where generic catching is truly necessary. Use `// noqa: console` only for debugging that won't be committed.

---

## Logging

**DO NOT use `console.*` methods.** Always use the structured logger.

```typescript
import { getLogger } from '@/utils/logger';

const logger = getLogger('ComponentName');

// ❌ BAD
console.log('User logged in');
console.error('Failed to save', error);

// ✅ GOOD
logger.info('User logged in', { userId: user.id });
logger.error('Failed to save data', { error, userId: user.id });
```

**Log Levels:**
- `debug` - Detailed diagnostic information (dev mode only)
- `info` - General informational messages
- `warn` - Warning messages for recoverable issues
- `error` - Error messages for failures

**Context:** Always include relevant context in log messages as the second parameter.

---

## Component Size

**Keep components under 300 lines.** Split large components into smaller, focused modules.

```typescript
// ❌ BAD - 1000+ line component with everything inline
export default function App() {
  // 50 useState declarations
  // API calls
  // Business logic
  // Event handlers
  // Rendering logic
  // ... 1000 more lines
}

// ✅ GOOD - Separated concerns
export default function App() {
  // Use custom hooks for business logic
  const { status, fetchStatus } = useStatus();
  const { models, fetchModels } = useModels();

  // Use sub-components for UI sections
  return (
    <div>
      <Header />
      <StatusSection status={status} />
      <ModelSection models={models} />
    </div>
  );
}
```

**Extract:**
- Business logic → Custom hooks (`hooks/`)
- API calls → API client (`api/`)
- UI sections → Sub-components (`components/`)
- Utility functions → Utils (`utils/`)

**Pre-commit Hook:** Automatically checks file size and warns on >300 lines.

---

## File Organization

Organize code by feature and concern:

```
frontend/src/
├── api/                    # API clients
│   ├── pywebview.ts       # PyWebView API wrapper
│   ├── models.ts          # Model-related API calls
│   └── versions.ts        # Version-related API calls
├── components/            # React components
│   ├── Header/           # Component folder (if >1 file)
│   │   ├── Header.tsx
│   │   ├── Header.test.tsx
│   │   └── index.ts
│   ├── ModelManager.tsx   # Single-file component
│   └── ...
├── hooks/                 # Custom React hooks
│   ├── useStatus.ts
│   ├── useModels.ts
│   └── useVersions.ts
├── errors/                # Error classes
│   └── index.ts
├── types/                 # TypeScript type definitions
│   ├── api.ts            # API response types
│   ├── models.ts         # Model types
│   └── pywebview.d.ts    # PyWebView API types
├── utils/                 # Utility functions
│   ├── logger.ts
│   └── formatters.ts
└── App.tsx               # Main app component (<300 lines)
```

**Rules:**
- One component per file
- Co-locate tests with source files
- Use index.ts for folder exports
- Group related files in folders

---

## Pre-commit Checks

Before committing, the following checks run automatically:

1. **ESLint** - Code quality and style
2. **Type Check** - TypeScript compilation
3. **Error Handling** - Proper exception handling
4. **File Size** - Component size limits

Run manually:
```bash
npm run precommit
```

Or individual checks:
```bash
npm run lint          # ESLint
npm run check:types   # TypeScript
npm run check:errors  # Error handling
npm run check:size    # File size
```

**Fix issues before committing.** The pre-commit hook will block commits with violations.

---

## Quick Reference

**Before writing code:**
- Understand the file organization
- Review existing patterns
- Check for similar implementations

**While writing code:**
- Use logger, not console.*
- Add proper TypeScript types (no `any`)
- Use specific error types from `@/errors`
- Keep components under 300 lines
- Extract logic into hooks/utils

**Before committing:**
- Run `npm run precommit`
- Fix all ESLint warnings
- Fix all TypeScript errors
- Ensure tests pass

**The pre-commit hooks will catch most issues automatically!**
```

**Success Criteria:**
- [ ] CONTRIBUTING.md created
- [ ] All standards documented
- [ ] Examples provided for each rule

---

## Phase 2: Create Type Definitions

### 2.1 PyWebView API Types

**File:** `frontend/src/types/pywebview.d.ts`

Replace all `any` types in the global Window interface with proper types:

```typescript
export interface DiskSpaceResponse {
  success: boolean;
  total: number;
  used: number;
  free: number;
  percent: number;
  error?: string;
}

export interface StatusResponse {
  success: boolean;
  version: string;
  deps_ready: boolean;
  patched: boolean;
  menu_shortcut: boolean;
  desktop_shortcut: boolean;
  shortcut_version: string | null;
  message: string;
  comfyui_running: boolean;
  last_launch_error: string | null;
  last_launch_log: string | null;
  app_resources?: {
    comfyui?: {
      gpu_memory?: number;
      ram_memory?: number;
    };
  };
}

export interface VersionInfo {
  tag_name: string;
  published_at: string;
  body: string;
  assets: Array<{
    name: string;
    size: number;
    download_url: string;
  }>;
}

export interface InstallationProgress {
  tag?: string;
  status: 'idle' | 'downloading' | 'extracting' | 'installing' | 'complete' | 'error';
  progress: number;
  message: string;
  error?: string;
}

export interface ShortcutState {
  menu: boolean;
  desktop: boolean;
  tag: string;
}

export interface ModelData {
  modelType: string;
  officialName?: string;
  cleanedName?: string;
  size?: number;
  addedDate?: string;
}

export interface ModelsResponse {
  success: boolean;
  models: Record<string, ModelData>;
  error?: string;
}

export interface LaunchResponse {
  success: boolean;
  error?: string;
  log_path?: string;
  ready?: boolean;
}

export interface CacheStatus {
  has_cache: boolean;
  is_valid: boolean;
  is_fetching: boolean;
  age_seconds?: number;
  last_fetched?: string;
  releases_count?: number;
}

// Main PyWebView API interface
export interface PyWebViewAPI {
  // Status & System
  get_status(): Promise<StatusResponse>;
  get_disk_space(): Promise<DiskSpaceResponse>;
  get_system_resources(): Promise<{
    success: boolean;
    resources: SystemResources;
    error?: string;
  }>;

  // Dependencies
  install_deps(): Promise<{ success: boolean }>;

  // Shortcuts
  toggle_menu(tag?: string): Promise<{ success: boolean }>;
  toggle_desktop(tag?: string): Promise<{ success: boolean }>;
  get_version_shortcuts(tag: string): Promise<{
    success: boolean;
    state: ShortcutState;
    error?: string;
  }>;
  get_all_shortcut_states(): Promise<{
    success: boolean;
    states: {
      active: string | null;
      states: Record<string, ShortcutState>;
    };
    error?: string;
  }>;
  set_version_shortcuts(tag: string, enabled: boolean): Promise<{
    success: boolean;
    state: ShortcutState;
    error?: string;
  }>;
  toggle_version_menu(tag: string): Promise<{
    success: boolean;
    state: ShortcutState;
    error?: string;
  }>;
  toggle_version_desktop(tag: string): Promise<{
    success: boolean;
    state: ShortcutState;
    error?: string;
  }>;

  // Version Management
  get_available_versions(force_refresh?: boolean): Promise<{
    success: boolean;
    versions: VersionInfo[];
    error?: string;
  }>;
  get_installed_versions(): Promise<{
    success: boolean;
    versions: string[];
    error?: string;
  }>;
  validate_installations(): Promise<{
    success: boolean;
    result: {
      had_invalid: boolean;
      removed: string[];
      valid: string[];
    };
    error?: string;
  }>;
  get_installation_progress(): Promise<InstallationProgress>;
  install_version(tag: string): Promise<{ success: boolean; error?: string }>;
  cancel_installation(): Promise<{ success: boolean; error?: string }>;
  remove_version(tag: string): Promise<{ success: boolean; error?: string }>;
  switch_version(tag: string): Promise<{ success: boolean; error?: string }>;
  get_active_version(): Promise<{ success: boolean; version: string; error?: string }>;
  check_version_dependencies(tag: string): Promise<{
    success: boolean;
    dependencies: Record<string, unknown>;
    error?: string;
  }>;
  install_version_dependencies(tag: string): Promise<{ success: boolean; error?: string }>;
  get_version_status(): Promise<{
    success: boolean;
    status: Record<string, unknown>;
    error?: string;
  }>;
  get_version_info(tag: string): Promise<{
    success: boolean;
    info: VersionInfo;
    error?: string;
  }>;
  launch_version(tag: string, extra_args?: string[]): Promise<LaunchResponse>;
  get_default_version(): Promise<{ success: boolean; version: string; error?: string }>;
  set_default_version(tag?: string | null): Promise<{ success: boolean; error?: string }>;

  // Size Calculation
  calculate_release_size(tag: string, force_refresh?: boolean): Promise<{
    success: boolean;
    total_bytes?: number;
    error?: string;
  }>;
  calculate_all_release_sizes(): Promise<{
    success: boolean;
    sizes?: Record<string, number>;
    error?: string;
  }>;

  // Utility
  open_url(url: string): Promise<{ success: boolean; error?: string }>;
  open_path(path: string): Promise<{ success: boolean; error?: string }>;
  close_window(): Promise<{ success: boolean }>;

  // Process Management
  launch_comfyui(): Promise<LaunchResponse>;
  stop_comfyui(): Promise<{ success: boolean }>;

  // Resource Management
  get_models(): Promise<ModelsResponse>;
  get_custom_nodes(version_tag: string): Promise<{
    success: boolean;
    nodes: string[];
    error?: string;
  }>;
  install_custom_node(
    git_url: string,
    version_tag: string,
    node_name?: string
  ): Promise<{ success: boolean; error?: string }>;
  update_custom_node(
    node_name: string,
    version_tag: string
  ): Promise<{ success: boolean; error?: string }>;
  remove_custom_node(
    node_name: string,
    version_tag: string
  ): Promise<{ success: boolean; error?: string }>;
  scan_shared_storage(): Promise<{
    success: boolean;
    result: {
      modelsFound?: number;
      [key: string]: unknown;
    };
    error?: string;
  }>;

  // Model Downloads
  download_model_from_hf(
    repo_id: string,
    family: string,
    official_name: string,
    model_type?: string | null,
    subtype?: string | null,
    quant?: string | null
  ): Promise<{ success: boolean; model_path?: string; error?: string }>;
  start_model_download_from_hf(
    repo_id: string,
    family: string,
    official_name: string,
    model_type?: string | null,
    subtype?: string | null,
    quant?: string | null
  ): Promise<{
    success: boolean;
    download_id?: string;
    total_bytes?: number;
    error?: string;
  }>;
  get_model_download_status(download_id: string): Promise<{
    success: boolean;
    download_id?: string;
    repo_id?: string;
    status?: string;
    progress?: number;
    downloaded_bytes?: number;
    total_bytes?: number;
    error?: string;
  }>;
  cancel_model_download(download_id: string): Promise<{ success: boolean; error?: string }>;
  search_hf_models(query: string, kind?: string | null, limit?: number): Promise<{
    success: boolean;
    models: Array<Record<string, unknown>>;
    error?: string;
  }>;

  // Launcher Updates
  get_launcher_version(): Promise<{
    success: boolean;
    version: string;
    branch: string;
    isGitRepo: boolean;
    error?: string;
  }>;
  check_launcher_updates(force_refresh?: boolean): Promise<{
    success: boolean;
    hasUpdate: boolean;
    currentCommit: string;
    latestCommit: string;
    commitsBehind: number;
    commits: Array<Record<string, unknown>>;
    error?: string;
  }>;
  apply_launcher_update(): Promise<{
    success: boolean;
    message: string;
    newCommit?: string;
    error?: string;
  }>;
  restart_launcher(): Promise<{ success: boolean; message: string; error?: string }>;

  // Cache Status
  get_github_cache_status(): Promise<{
    success: boolean;
    status: CacheStatus;
    error?: string;
  }>;
  has_background_fetch_completed(): Promise<{
    success: boolean;
    completed: boolean;
    error?: string;
  }>;
  reset_background_fetch_flag(): Promise<{
    success: boolean;
    error?: string;
  }>;
}

// Global Window extension
declare global {
  interface Window {
    pywebview?: {
      api: PyWebViewAPI;
    };
  }
}

export {};
```

**Success Criteria:**
- [ ] All PyWebView API methods properly typed
- [ ] All response interfaces defined
- [ ] No `any` types in API definitions
- [ ] Global Window interface extended

---

### 2.2 API Response Types

**File:** `frontend/src/types/api.ts`

Common API response patterns:

```typescript
export interface BaseResponse {
  success: boolean;
  error?: string;
}

export interface ProgressStatus {
  status: 'idle' | 'in-progress' | 'complete' | 'error';
  progress: number;
  message: string;
}

export interface NetworkStatus {
  online: boolean;
  latency?: number;
}

export type AsyncOperationStatus =
  | { state: 'idle' }
  | { state: 'loading' }
  | { state: 'success'; data: unknown }
  | { state: 'error'; error: Error };
```

**Success Criteria:**
- [ ] Common response types defined
- [ ] Type aliases for complex types created
- [ ] All types exported properly

---

## Phase 3: Refactor Monolithic Files

### 3.1 Create API Client Layer

**Target:** Extract all PyWebView API calls from App.tsx

**File:** `frontend/src/api/pywebview.ts`

```typescript
import { getLogger } from '@/utils/logger';
import { APIError, NetworkError } from '@/errors';
import type {
  PyWebViewAPI,
  StatusResponse,
  DiskSpaceResponse,
  ModelsResponse,
  LaunchResponse,
} from '@/types/pywebview';

const logger = getLogger('PyWebViewClient');

class PyWebViewClient {
  private get api(): PyWebViewAPI {
    if (!window.pywebview?.api) {
      throw new APIError('PyWebView API not available');
    }
    return window.pywebview.api;
  }

  async getStatus(): Promise<StatusResponse> {
    try {
      logger.debug('Fetching system status');
      const response = await this.api.get_status();

      if (!response.success) {
        throw new APIError('Status fetch failed', 'get_status');
      }

      logger.info('Status fetched successfully', { version: response.version });
      return response;
    } catch (error) {
      if (error instanceof APIError) {
        throw error;
      }
      logger.error('Failed to fetch status', { error });
      throw new APIError('Unexpected error fetching status', 'get_status', error as Error);
    }
  }

  async getDiskSpace(): Promise<DiskSpaceResponse> {
    try {
      logger.debug('Fetching disk space');
      const response = await this.api.get_disk_space();

      if (!response.success) {
        throw new APIError(response.error || 'Disk space fetch failed', 'get_disk_space');
      }

      logger.debug('Disk space fetched', { percent: response.percent });
      return response;
    } catch (error) {
      if (error instanceof APIError) {
        throw error;
      }
      logger.error('Failed to fetch disk space', { error });
      throw new APIError('Unexpected error fetching disk space', 'get_disk_space', error as Error);
    }
  }

  async getModels(): Promise<ModelsResponse> {
    try {
      logger.debug('Fetching models');
      const response = await this.api.get_models();

      if (!response.success) {
        throw new APIError(response.error || 'Models fetch failed', 'get_models');
      }

      const modelCount = Object.keys(response.models).length;
      logger.info('Models fetched successfully', { count: modelCount });
      return response;
    } catch (error) {
      if (error instanceof APIError) {
        throw error;
      }
      logger.error('Failed to fetch models', { error });
      throw new APIError('Unexpected error fetching models', 'get_models', error as Error);
    }
  }

  async launchComfyUI(): Promise<LaunchResponse> {
    try {
      logger.info('Launching ComfyUI');
      const response = await this.api.launch_comfyui();

      if (!response.success) {
        throw new APIError(response.error || 'Launch failed', 'launch_comfyui');
      }

      logger.info('ComfyUI launched successfully', { logPath: response.log_path });
      return response;
    } catch (error) {
      if (error instanceof APIError) {
        throw error;
      }
      logger.error('Failed to launch ComfyUI', { error });
      throw new APIError('Unexpected error launching ComfyUI', 'launch_comfyui', error as Error);
    }
  }

  async stopComfyUI(): Promise<void> {
    try {
      logger.info('Stopping ComfyUI');
      const response = await this.api.stop_comfyui();

      if (!response.success) {
        throw new APIError('Stop failed', 'stop_comfyui');
      }

      logger.info('ComfyUI stopped successfully');
    } catch (error) {
      if (error instanceof APIError) {
        throw error;
      }
      logger.error('Failed to stop ComfyUI', { error });
      throw new APIError('Unexpected error stopping ComfyUI', 'stop_comfyui', error as Error);
    }
  }

  async openPath(path: string): Promise<void> {
    try {
      logger.debug('Opening path', { path });
      const response = await this.api.open_path(path);

      if (!response.success) {
        throw new APIError(response.error || 'Failed to open path', 'open_path');
      }

      logger.info('Path opened successfully', { path });
    } catch (error) {
      if (error instanceof APIError) {
        throw error;
      }
      logger.error('Failed to open path', { path, error });
      throw new APIError('Unexpected error opening path', 'open_path', error as Error);
    }
  }

  isAvailable(): boolean {
    return !!window.pywebview?.api;
  }
}

export const pywebview = new PyWebViewClient();
```

**Additional API Files:**

- `frontend/src/api/versions.ts` - Version management API calls
- `frontend/src/api/models.ts` - Model management API calls
- `frontend/src/api/launcher.ts` - Launcher update API calls

**Success Criteria:**
- [ ] PyWebView client created with proper error handling
- [ ] All API methods typed and logged
- [ ] Singleton instance exported
- [ ] Additional API modules created

---

### 3.2 Extract Custom Hooks

**Target:** Move business logic from App.tsx into reusable hooks

**File:** `frontend/src/hooks/useStatus.ts`

```typescript
import { useState, useEffect, useRef } from 'react';
import { getLogger } from '@/utils/logger';
import { pywebview } from '@/api/pywebview';
import { APIError } from '@/errors';
import type { StatusResponse } from '@/types/pywebview';

const logger = getLogger('useStatus');

export function useStatus(pollInterval = 500) {
  const [status, setStatus] = useState<StatusResponse | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const isPolling = useRef(false);

  const fetchStatus = async (): Promise<void> => {
    if (isPolling.current) {
      return;
    }

    isPolling.current = true;

    try {
      const response = await pywebview.getStatus();
      setStatus(response);
      setError(null);
    } catch (err) {
      if (err instanceof APIError) {
        logger.error('Status fetch failed', { error: err.message });
        setError(err);
      } else {
        logger.error('Unexpected error fetching status', { err });
        setError(new APIError('Unexpected error'));
      }
    } finally {
      setIsLoading(false);
      isPolling.current = false;
    }
  };

  useEffect(() => {
    // Initial fetch
    void fetchStatus();

    // Set up polling
    const interval = setInterval(() => {
      void fetchStatus();
    }, pollInterval);

    return () => clearInterval(interval);
  }, [pollInterval]);

  return {
    status,
    isLoading,
    error,
    refetch: fetchStatus,
  };
}
```

**Additional Hook Files:**

- `frontend/src/hooks/useModels.ts` - Model fetching and management
- `frontend/src/hooks/useDiskSpace.ts` - Disk space monitoring
- `frontend/src/hooks/useLauncherUpdate.ts` - Launcher update logic
- `frontend/src/hooks/useComfyUIProcess.ts` - ComfyUI process management

**Success Criteria:**
- [ ] Status polling extracted into hook
- [ ] All hooks properly typed
- [ ] Error handling implemented
- [ ] Logging added to all hooks

---

### 3.3 Split App.tsx into Components

**Target:** Break down 1024-line App.tsx into focused components

**New Structure:**

```
frontend/src/components/
├── ComfyUIManager/
│   ├── ComfyUIManager.tsx      # Main component (< 200 lines)
│   ├── DependencySection.tsx   # Dependency installation UI
│   ├── ControlPanel.tsx        # Launch controls
│   └── index.ts
├── StatusDisplay/
│   ├── StatusDisplay.tsx
│   └── index.ts
└── ... (existing components)
```

**File:** `frontend/src/components/ComfyUIManager/ComfyUIManager.tsx`

```typescript
import React from 'react';
import { getLogger } from '@/utils/logger';
import { Header } from '../Header';
import { AppSidebar } from '../AppSidebar';
import { VersionSelector } from '../VersionSelector';
import { DependencySection } from './DependencySection';
import { ControlPanel } from './ControlPanel';
import { useStatus } from '@/hooks/useStatus';
import { useVersions } from '@/hooks/useVersions';
import { useModels } from '@/hooks/useModels';

const logger = getLogger('ComfyUIManager');

export function ComfyUIManager(): JSX.Element {
  const { status, isLoading: statusLoading } = useStatus();
  const { installedVersions, activeVersion, switchVersion } = useVersions();
  const { models } = useModels();

  logger.debug('Rendering ComfyUIManager', {
    statusLoading,
    installedCount: installedVersions.length,
  });

  return (
    <div className="w-full h-screen gradient-bg-blobs flex flex-col relative overflow-hidden font-mono">
      <Header
        systemResources={status?.app_resources}
        launcherUpdateAvailable={false}
        onClose={() => window.close()}
      />

      <div className="flex flex-1 relative z-10 overflow-hidden">
        <AppSidebar /* props */ />

        <div className="flex-1 flex flex-col overflow-hidden">
          <VersionSelector
            installedVersions={installedVersions}
            activeVersion={activeVersion}
            onSwitch={switchVersion}
          />

          <DependencySection
            depsInstalled={status?.deps_ready ?? false}
            isLoading={statusLoading}
          />

          <ControlPanel
            comfyUIRunning={status?.comfyui_running ?? false}
            depsReady={status?.deps_ready ?? false}
          />
        </div>
      </div>
    </div>
  );
}
```

**File:** `frontend/src/components/ComfyUIManager/DependencySection.tsx`

Extract dependency installation UI (~50 lines)

**File:** `frontend/src/components/ComfyUIManager/ControlPanel.tsx`

Extract control panel UI (~50 lines)

**Success Criteria:**
- [ ] App.tsx reduced to < 200 lines
- [ ] Components extracted and focused
- [ ] Each component properly typed
- [ ] Logging added to components

---

### 3.4 Refactor Other Large Files

**Files to Refactor:**

1. **ModelManager.tsx (986 lines)** → Target: < 300 lines
   - Extract search logic into `useModelSearch` hook
   - Split download UI into `ModelDownloadDialog` component
   - Create `ModelList` and `ModelCard` sub-components

2. **InstallDialog.tsx (939 lines)** → Target: < 300 lines
   - Extract installation logic into `useInstallation` hook
   - Split version list into `VersionList` component
   - Create `InstallationProgress` component

3. **useVersions.ts (728 lines)** → Target: < 300 lines
   - Split into multiple focused hooks:
     - `useVersionList.ts` - Version fetching
     - `useVersionInstall.ts` - Installation logic
     - `useVersionSwitch.ts` - Switching logic

**Success Criteria:**
- [ ] All files under 300 lines
- [ ] Logic properly separated
- [ ] No duplicate code
- [ ] All modules properly typed

---

## Phase 4: Apply Error Handling Standards

### 4.1 Update All Catch Blocks

**Target:** Replace generic error handling throughout codebase

**Pattern to Find and Replace:**

```typescript
// ❌ BEFORE
try {
  await operation();
} catch (e) {
  console.error('Failed:', e);
}

// ✅ AFTER
try {
  await operation();
} catch (error) {
  if (error instanceof NetworkError) {
    logger.error('Network operation failed', { url: error.url, status: error.status });
    throw new APIError('Operation failed due to network error', undefined, error);
  }
  if (error instanceof ValidationError) {
    logger.error('Validation failed', { field: error.field });
    throw error;
  }
  logger.error('Unexpected error in operation', { error });
  throw error;
}
```

**Files to Update:**
- All API client files
- All hook files
- All component files with async operations

**Success Criteria:**
- [ ] No generic catch blocks remain
- [ ] All catch blocks use type guards
- [ ] All errors properly logged
- [ ] Pre-commit hook passes

---

### 4.2 Replace Console Usage

**Target:** Replace all `console.*` with structured logger

**Search and Replace:**
- `console.log` → `logger.info`
- `console.error` → `logger.error`
- `console.warn` → `logger.warn`
- `console.debug` → `logger.debug`

**Success Criteria:**
- [ ] No console.* usage in src/
- [ ] All logging uses getLogger()
- [ ] Pre-commit hook passes

---

## Phase 5: Validation and Testing

### 5.1 Type Check

```bash
cd frontend
npm run check:types
```

**Fix all TypeScript errors:**
- No implicit any
- No missing return types
- No null/undefined issues

**Success Criteria:**
- [ ] Zero TypeScript errors
- [ ] Zero TypeScript warnings
- [ ] Strict mode enabled

---

### 5.2 Linting

```bash
cd frontend
npm run lint
```

**Fix all ESLint errors:**
- No console usage
- No generic Error usage
- No file size violations
- No complexity violations

**Success Criteria:**
- [ ] Zero ESLint errors
- [ ] Zero ESLint warnings (or documented exceptions)

---

### 5.3 Pre-commit Checks

```bash
cd frontend
npm run precommit
```

**All checks must pass:**
- ESLint
- TypeScript
- Error handling
- File size

**Success Criteria:**
- [ ] All pre-commit checks passing
- [ ] No violations found

---

### 5.4 Update Tests

**For each refactored module:**
- Update existing tests
- Add tests for error handling
- Add tests for edge cases

**Coverage target:** ≥80% for all new/refactored code

**Success Criteria:**
- [ ] All tests passing
- [ ] Coverage targets met
- [ ] Error handling tested

---

## Phase 6: Documentation and Cleanup

### 6.1 Update README

**File:** `frontend/README.md`

Document:
- New project structure
- Coding standards
- How to run pre-commit checks
- How to add new components/hooks

**Success Criteria:**
- [ ] README updated
- [ ] Examples provided
- [ ] Standards referenced

---

### 6.2 Remove Deprecated Files

**Files to Remove:**
- `App.tsx.backup`
- `App.new.tsx`
- Any other `.backup` files

**Success Criteria:**
- [ ] Backup files removed
- [ ] No dead code remaining

---

### 6.3 Git Commit

**Commit Message Format:**

```
feat: refactor frontend to match backend coding standards

- Add strict TypeScript configuration
- Implement custom error hierarchy
- Create API client layer with proper typing
- Extract business logic into custom hooks
- Split monolithic components (App.tsx, ModelManager.tsx, InstallDialog.tsx)
- Add pre-commit hooks for error handling and file size
- Replace console.* with structured logger
- Add frontend CONTRIBUTING.md

All files now under 300 lines, fully typed, with proper error handling.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
```

**Success Criteria:**
- [ ] All changes committed
- [ ] Commit message follows convention
- [ ] Pre-commit hooks passed

---

## Success Metrics

### Before Refactoring
- ❌ App.tsx: 1024 lines
- ❌ ModelManager.tsx: 986 lines
- ❌ InstallDialog.tsx: 939 lines
- ❌ useVersions.ts: 728 lines
- ❌ 16 instances of `any` in App.tsx
- ❌ Generic error handling throughout
- ❌ console.* usage mixed with logger
- ❌ No pre-commit hooks
- ❌ Lax TypeScript configuration

### After Refactoring
- ✅ All files < 300 lines
- ✅ Zero `any` types (except documented exceptions)
- ✅ Specific error types with logging
- ✅ Consistent logger usage
- ✅ Pre-commit hooks enforcing standards
- ✅ Strict TypeScript configuration
- ✅ Comprehensive type definitions
- ✅ Documented coding standards

---

## Timeline Estimate

**Phase 1:** Establish Standards - 2-3 sessions
**Phase 2:** Type Definitions - 1-2 sessions
**Phase 3:** Refactor Monoliths - 4-6 sessions
**Phase 4:** Error Handling - 2-3 sessions
**Phase 5:** Validation - 1-2 sessions
**Phase 6:** Documentation - 1 session

**Total:** ~11-17 coding sessions

---

## Getting Started

To begin this refactoring:

1. **Review this plan** - Understand the scope and approach
2. **Start with Phase 1.1** - Create error hierarchy (quick win)
3. **Enable strict TypeScript** - Phase 1.2 (will reveal issues to fix)
4. **Work incrementally** - One phase at a time
5. **Test frequently** - Don't accumulate too many changes

**First PR should include:**
- Error hierarchy (`src/errors/index.ts`)
- Strict TypeScript config
- Enhanced ESLint rules
- Pre-commit scripts
- Frontend CONTRIBUTING.md

This establishes the foundation for all subsequent refactoring work.
