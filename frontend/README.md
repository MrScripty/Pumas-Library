# Frontend Architecture

This document explains the architecture, design decisions, and organization of the ComfyUI Launcher frontend.

## Overview

The frontend is a React-based single-page application (SPA) that provides:
- Multi-app launcher with visual status indicators
- Version selection and installation UI
- Model management interface (search, download, import, mapping)
- System resource monitoring display
- Installation progress tracking
- Real-time status updates via PyWebView bridge

## Technology Stack

- **React 18** - UI library with hooks
- **TypeScript** - Type-safe JavaScript with strict mode
- **Vite** - Fast build tool and dev server
- **Tailwind CSS v4** - Utility-first CSS framework
- **Framer Motion** - Animation library
- **React Aria** - Accessible interaction hooks
- **Lucide React** - Icon library

## Architecture Patterns

### 1. PyWebView Bridge Pattern

The frontend communicates with the Python backend via PyWebView's JavaScript API:

```typescript
// Type-safe API declarations
declare global {
  interface Window {
    pywebview: {
      api: {
        get_releases(options?: ReleaseOptions): Promise<Release[]>;
        install_version(tag: string): Promise<boolean>;
        launch_version(tag: string): Promise<boolean>;
        // ... more API methods
      };
    };
  }
}
```

**Design Rationale:**
- **Type safety**: Full TypeScript definitions for Python API
- **Async by default**: All backend calls return Promises
- **Error handling**: Errors propagated to frontend for user feedback
- **Separation**: UI logic separate from backend business logic

See [src/types/pywebview.d.ts](src/types/pywebview.d.ts) for API type definitions.

### 2. Component Architecture

Components are organized by feature and responsibility:

```
src/
├── components/          # React components
│   ├── AppIcon.tsx      # Multi-state app icon with animations
│   ├── AppSidebar.tsx   # App switcher sidebar
│   ├── InstallDialog.tsx # Installation UI
│   ├── ModelManager.tsx  # Model management UI
│   ├── VersionSelector.tsx # Version selection dropdown
│   └── StatusFooter.tsx  # Status bar with system info
├── hooks/               # Custom React hooks
│   ├── useVersions.ts   # Version data management
│   ├── useModels.ts     # Model library state
│   ├── useStatus.ts     # Status polling
│   └── ...
├── api/                 # API abstraction layer
│   ├── pywebview.ts     # PyWebView API wrappers
│   ├── versions.ts      # Version management API
│   └── models.ts        # Model library API
├── types/               # TypeScript type definitions
├── errors/              # Custom error classes
├── utils/               # Utility functions
└── config/              # Configuration constants
```

**Design Rationale:**
- **Single Responsibility**: Each component handles one concern
- **Reusability**: Shared logic extracted to hooks
- **Type Safety**: All components fully typed
- **Testability**: Components designed for easy testing

### 3. State Management

State managed using React hooks with clear data flow:

```typescript
// Custom hooks encapsulate state logic
function useVersions() {
  const [releases, setReleases] = useState<Release[]>([]);
  const [loading, setLoading] = useState(false);

  const fetchReleases = useCallback(async () => {
    setLoading(true);
    try {
      const data = await window.pywebview.api.get_releases();
      setReleases(data);
    } catch (error) {
      // Error handling
    } finally {
      setLoading(false);
    }
  }, []);

  return { releases, loading, fetchReleases };
}
```

**State Organization:**
- **Local state**: Component-specific UI state (hover, focus, etc.)
- **Lifted state**: Shared state in parent components
- **Custom hooks**: Reusable state logic
- **No global store**: React hooks sufficient for current complexity

**Design Rationale:**
- **Simplicity**: No Redux/MobX overhead needed
- **Colocation**: State lives near where it's used
- **Performance**: Fine-grained re-renders with hooks
- **Type safety**: Full TypeScript inference

### 4. Error Handling

Custom error hierarchy matches backend pattern:

```typescript
// src/errors/index.ts
export class ComfyUILauncherError extends Error {
  constructor(message: string, public cause?: Error) {
    super(message);
    this.name = this.constructor.name;
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
```

**Usage Pattern:**
```typescript
try {
  await window.pywebview.api.install_version(tag);
} catch (error) {
  if (error instanceof NetworkError) {
    logger.error('Network request failed', { url: error.url });
    throw new APIError(`Failed to install: ${error.message}`, 'install_version', error);
  } else if (error instanceof ValidationError) {
    logger.error('Invalid input', { field: error.field });
    // Handle validation error
  } else {
    throw error; // Re-throw unknown errors
  }
}
```

**Design Rationale:**
- **Specific handling**: Catch specific error types, not generic `Error`
- **Cause chaining**: Preserve error context
- **Type safety**: Custom errors carry relevant data
- **Logging required**: All catch blocks must log

See [CONTRIBUTING.md](CONTRIBUTING.md) for error handling standards.

### 5. Theme System

Dark theme built on CSS custom properties with semantic naming:

```css
/* index.css */
:root {
  /* Surface colors (organized by elevation) */
  --surface-lowest: 217 33% 6%;
  --surface-low: 217 33% 8%;
  --surface-mid: 217 32% 11%;
  --surface-interactive: 217 32% 16%;
  --surface-interactive-hover: 217 32% 20%;

  /* Text hierarchy */
  --text-primary: 210 40% 96%;
  --text-secondary: 210 20% 62%;
  --text-tertiary: 210 13% 40%;

  /* Semantic accents */
  --accent-success: 142 100% 66%;
  --accent-error: 0 84% 60%;
  --accent-warning: 38 92% 50%;
  --accent-info: 199 89% 48%;
}
```

**Usage:**
```tsx
// Using CSS variables
<div className="bg-[hsl(var(--surface-interactive))]">

// Using utility classes (preferred)
<div className="surface-interactive">

// Type-safe config (for programmatic use)
import { themeColors } from '@/config/theme';
<div style={{ backgroundColor: themeColors.surfaces.interactive }}>
```

**Design Rationale:**
- **Semantic naming**: Color purpose clear from name
- **Consistency**: All components use same tokens
- **Maintainability**: Change theme once, updates everywhere
- **Type safety**: TypeScript config for programmatic access

See [THEME_SYSTEM.md](THEME_SYSTEM.md) for complete theme documentation.

### 6. Accessibility

Accessibility enforced via React Aria hooks:

```typescript
import { useHover } from '@react-aria/interactions';

function Component() {
  const { hoverProps, isHovered } = useHover({});

  return (
    <div {...hoverProps}>
      {isHovered ? 'Hovering!' : 'Not hovering'}
    </div>
  );
}
```

**Prohibited Patterns:**
- ❌ `onMouseEnter` / `onMouseLeave` - Use `useHover` instead
- ❌ `onClick` without keyboard equivalent - Use `usePress` instead
- ❌ Non-semantic interactive elements - Use proper HTML elements

**Design Rationale:**
- **Reliability**: React Aria handles edge cases (fast movements, window blur)
- **Accessibility**: Keyboard navigation and screen reader support
- **Cross-platform**: Works correctly on touch devices
- **Enforcement**: ESLint rules prevent raw event handlers

See [docs/REACT_ARIA_ENFORCEMENT.md](../docs/REACT_ARIA_ENFORCEMENT.md) for details.

### 7. Animation Strategy

Framer Motion provides physics-based animations:

```typescript
import { motion } from 'framer-motion';

<motion.div
  layout  // Automatic layout animations
  initial={{ opacity: 0, y: 20 }}
  animate={{ opacity: 1, y: 0 }}
  exit={{ opacity: 0, y: -20 }}
  transition={{ duration: 0.2 }}
>
  Content
</motion.div>
```

**Animation Guidelines:**
- **Layout animations**: Use `layout` prop for position changes
- **Enter/exit**: Use `initial`/`animate`/`exit` for mount/unmount
- **Gesture**: Use `whileHover`/`whileTap` for interactive feedback
- **Performance**: Prefer `transform` and `opacity` (GPU-accelerated)

**Design Rationale:**
- **UX polish**: Smooth transitions reduce cognitive load
- **Feedback**: Animations indicate state changes
- **Performance**: GPU acceleration keeps UI responsive
- **Accessibility**: Respects `prefers-reduced-motion`

### 8. Multi-App System

Extensible app configuration for supporting multiple applications:

```typescript
// config/apps.ts
export const DEFAULT_APPS: AppConfig[] = [
  {
    id: 'comfyui',
    name: 'ComfyUI',
    description: 'Node-based Stable Diffusion GUI',
    color: '#ff6b6b',
    defaultPort: 8188,
    category: 'diffusion'
  },
  {
    id: 'open-webui',
    name: 'Open WebUI',
    description: 'ChatGPT-style LLM interface',
    color: '#4ecdc4',
    defaultPort: 8080,
    category: 'llm'
  },
  // ... more apps
];
```

**Design Rationale:**
- **Extensibility**: Easy to add new apps
- **Consistency**: Same UI patterns for all apps
- **Flexibility**: Per-app configuration (ports, models, etc.)
- **Type safety**: Full TypeScript definitions

See [docs/architecture/MULTI_APP_SYSTEM.md](../docs/architecture/MULTI_APP_SYSTEM.md) for details.

## Component Guidelines

### Component Size

**Target:** <300 lines per component

**Refactoring triggers:**
- Component exceeds 300 lines
- Multiple responsibilities in one component
- Difficult to test or understand

**Refactoring approach:**
- Extract sub-components for logical sections
- Move state logic to custom hooks
- Split large components into feature-focused smaller ones

### Type Safety

**All components must:**
- Define prop interfaces
- Have no `any` types (use `unknown` if truly dynamic)
- Export prop types for consumers
- Use strict TypeScript mode

```typescript
// ✅ GOOD
interface ButtonProps {
  label: string;
  onClick: () => void;
  variant?: 'primary' | 'secondary';
}

export function Button({ label, onClick, variant = 'primary' }: ButtonProps) {
  // ...
}

// ❌ BAD - no types, uses any
export function Button({ label, onClick, variant }: any) {
  // ...
}
```

### Logging

Use structured logging instead of console.*:

```typescript
import { getLogger } from '@/utils/logger';

const logger = getLogger('ComponentName');

logger.debug('Detailed info', { data });
logger.info('User action', { action: 'click' });
logger.warning('Recoverable issue', { reason });
logger.error('Error occurred', { error });
```

**Design Rationale:**
- **Structured output**: Consistent log format
- **Filtering**: Can filter by component name
- **Production ready**: Can route to external logging service
- **Type safe**: Logger methods are typed

## Build Configuration

### Vite Configuration

```typescript
// vite.config.ts
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: 'dist',
    sourcemap: true,
    rollupOptions: {
      output: {
        manualChunks: {
          'react-vendor': ['react', 'react-dom'],
          'motion': ['framer-motion'],
        }
      }
    }
  }
});
```

**Optimization:**
- Code splitting for vendor dependencies
- Tree shaking for unused code
- Minification in production
- Source maps for debugging

### Development Server

```bash
npm run dev     # Start dev server (http://localhost:5173)
npm run build   # Production build
npm run preview # Preview production build
npm run lint    # Run ESLint
npm run typecheck # Run TypeScript compiler
```

## Testing

### Component Testing

```typescript
import { render, screen } from '@testing-library/react';
import { Button } from './Button';

test('renders button with label', () => {
  render(<Button label="Click me" onClick={() => {}} />);
  expect(screen.getByText('Click me')).toBeInTheDocument();
});

test('calls onClick when clicked', () => {
  const handleClick = jest.fn();
  render(<Button label="Click" onClick={handleClick} />);
  screen.getByText('Click').click();
  expect(handleClick).toHaveBeenCalledTimes(1);
});
```

**Testing Guidelines:**
- Test user behavior, not implementation
- Use `data-testid` for non-text elements
- Mock PyWebView API in tests
- Test error states and edge cases

## Performance Considerations

### Optimization Techniques

1. **Memoization**: Use `useMemo` for expensive calculations
2. **Callback stability**: Use `useCallback` to prevent re-renders
3. **Code splitting**: Dynamic imports for heavy components
4. **Virtualization**: For long lists (models, versions)
5. **Debouncing**: For search inputs and resize handlers

### Bundle Size

Current bundle sizes (gzipped):
- React vendor: ~45 KB
- Framer Motion: ~35 KB
- Application code: ~50 KB
- **Total**: ~130 KB (excellent)

**Monitoring:**
```bash
npm run build -- --analyze  # Visualize bundle composition
```

## Code Quality

### Linting

ESLint enforces:
- No `any` types
- No unused variables
- No missing dependencies in hooks
- React Aria usage (no raw mouse events)
- Accessibility rules (jsx-a11y)

```bash
npm run lint      # Check for violations
npm run lint:fix  # Auto-fix where possible
```

### Type Checking

TypeScript strict mode enabled:
```json
{
  "compilerOptions": {
    "strict": true,
    "noImplicitAny": true,
    "strictNullChecks": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true
  }
}
```

Run type checker:
```bash
npm run typecheck
```

## File Size Limits

Pre-commit hook enforces file size limits:
- Components: 20 KB (soft limit, warnings over 15 KB)
- Hooks: 10 KB
- Utils: 10 KB

**Rationale:** Large files are harder to understand and maintain.

## Related Documentation

- [CONTRIBUTING.md](CONTRIBUTING.md) - Frontend coding standards
- [THEME_SYSTEM.md](THEME_SYSTEM.md) - Theme system documentation
- [../docs/CODING_STANDARDS.md](../docs/CODING_STANDARDS.md) - General code standards
- [../docs/REACT_ARIA_ENFORCEMENT.md](../docs/REACT_ARIA_ENFORCEMENT.md) - React Aria usage
- [../docs/architecture/MULTI_APP_SYSTEM.md](../docs/architecture/MULTI_APP_SYSTEM.md) - Multi-app architecture
- [../docs/architecture/FRONTEND_ARCHITECTURE.md](../docs/architecture/FRONTEND_ARCHITECTURE.md) - Refactoring history and decisions

## Future Enhancements

Planned improvements:
- **Testing**: Add comprehensive component tests
- **Virtualization**: Implement virtual scrolling for model lists
- **Offline support**: Cache model metadata locally
- **Theme variants**: Support light theme and custom themes
- **Internationalization**: Add i18n support for multiple languages
