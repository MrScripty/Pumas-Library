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
│   └── versions.ts       # Version types
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
