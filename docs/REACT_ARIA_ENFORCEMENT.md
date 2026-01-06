# React Aria Enforcement Setup

This document describes how React Aria usage is enforced as a coding standard in this project.

## Overview

To prevent the hover state issues we experienced (where `onMouseLeave` doesn't fire reliably), we've implemented automated enforcement of React Aria hooks over raw DOM events.

## What's Enforced

The following DOM event handlers are **prohibited** via ESLint:
- `onMouseEnter` → Use `useHover` from `@react-aria/interactions`
- `onMouseLeave` → Use `useHover` from `@react-aria/interactions`
- `onMouseOver` → Use `useHover` from `@react-aria/interactions`
- `onMouseOut` → Use `useHover` from `@react-aria/interactions`

## Setup Components

### 1. ESLint Configuration (`eslint.config.js`)

Custom ESLint rules using `no-restricted-syntax` to detect and block mouse event handlers:

```javascript
'no-restricted-syntax': [
  'error',
  {
    selector: 'JSXAttribute[name.name="onMouseEnter"]',
    message: 'Avoid using onMouseEnter. Use React Aria\'s useHover hook...'
  },
  // ... similar rules for onMouseLeave, onMouseOver, onMouseOut
]
```

### 2. Accessibility Rules

We also enforce accessibility best practices via `eslint-plugin-jsx-a11y`:
- `jsx-a11y/mouse-events-have-key-events` - Ensures keyboard equivalents
- `jsx-a11y/no-static-element-interactions` - Warns about interactive non-semantic elements

### 3. Package Scripts

```bash
npm run lint        # Check for violations
npm run lint:fix    # Auto-fix where possible
```

## Running Linting

### Local Development

```bash
# Check for violations
npm run lint

# Auto-fix issues
npm run lint:fix
```

### CI/CD Integration

Add to your CI pipeline:

```yaml
- name: Lint
  run: |
    cd frontend
    npm run lint
```

### Pre-commit Hook (Optional)

You can add a pre-commit hook using husky:

```bash
npm install --save-dev husky lint-staged
npx husky init
```

Add to `.husky/pre-commit`:
```bash
cd frontend && npm run lint
```

## Handling Violations

When ESLint detects a mouse event violation, you'll see:

```
error  Avoid using onMouseEnter. Use React Aria's useHover hook from
       @react-aria/interactions for robust, accessible hover interactions
```

### Correct Approach

❌ **Before** (Prohibited):
```tsx
<div
  onMouseEnter={() => setHover(true)}
  onMouseLeave={() => setHover(false)}
>
  Content
</div>
```

✅ **After** (Enforced):
```tsx
import { useHover } from '@react-aria/interactions';

function Component() {
  const { hoverProps, isHovered } = useHover({});

  return <div {...hoverProps}>Content</div>;
}
```

## Exemptions

In rare cases where React Aria cannot be used, you can disable the rule:

```tsx
{/* eslint-disable-next-line no-restricted-syntax */}
<div onMouseEnter={handler}>
  {/* Justification: Legacy library integration requires raw events */}
</div>
```

**Important**: Exemptions require:
1. Code review approval
2. Clear justification in comments
3. Comprehensive testing for edge cases

## Why This Matters

React Aria's `useHover` solves critical issues that raw mouse events have:

1. **Reliability**: Handles fast mouse movements and window blur
2. **Accessibility**: Supports keyboard navigation and screen readers
3. **Cross-platform**: Works correctly on touch devices
4. **Browser consistency**: Normalizes behavior across browsers

See the [React Aria Documentation](https://react-spectrum.adobe.com/react-aria/useHover.html) for more details.

## Related Documentation

- [CODING_STANDARDS.md](./CODING_STANDARDS.md) - Full coding standards
- [React Aria useHover](https://react-spectrum.adobe.com/react-aria/useHover.html)
- [eslint-plugin-jsx-a11y](https://github.com/jsx-eslint/eslint-plugin-jsx-a11y)

## Maintenance

### Updating Rules

To add more restricted events, edit `eslint.config.js`:

```javascript
{
  selector: 'JSXAttribute[name.name="onFocus"]',
  message: 'Use React Aria\'s useFocus hook instead'
}
```

### Checking Coverage

To see all current violations in the codebase:

```bash
npm run lint
```

This will show files that need updating to comply with standards.
