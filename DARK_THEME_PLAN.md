# Dark Theme Implementation Plan

## Current State Analysis

### Theming Structure
The application currently uses **Tailwind CSS v4** (via `@tailwindcss/postcss`) with CSS custom properties for theming.

### Color System Location
All theme colors are defined in [frontend/src/index.css](frontend/src/index.css):
- Lines 5-42: Root CSS custom properties
- Lines 27-42: "Launcher theme tokens" (dark theme foundation already exists!)
- Lines 66-106: Tailwind theme integration

### Current Color Inconsistency
The **Version Selector** ([VersionSelector.tsx](frontend/src/components/VersionSelector.tsx)) uses hardcoded colors instead of the launcher theme tokens:

**Version Selector colors:**
- Background: `#2a2a2a` (line 359)
- Border: `#444` (line 359)
- Hover: `#333333` (lines 62, 65, etc.)
- Text: Various grays and whites
- Accent green: `#55ff55`

**Rest of GUI colors:**
- Uses `hsl(var(--launcher-bg-primary))`, `hsl(var(--launcher-bg-secondary))`, etc.
- StatusFooter: Uses standard Tailwind colors (blue-400, green-400, etc.)
- App body: `bg-[hsl(var(--launcher-bg-primary))]` (App.tsx:664)
- Sidebar: `bg-[hsl(var(--launcher-bg-secondary)/0.5)]` (AppSidebar.tsx:144)

---

## Implementation Plan

### Phase 1: Create Unified Dark Theme Token System

#### 1.1 Extend CSS Custom Properties
**File:** [frontend/src/index.css](frontend/src/index.css)

**Changes needed:**
```css
:root {
  /* Existing launcher tokens are good but need additions */

  /* Add specific component tokens based on version selector colors */
  --launcher-bg-control: 217 32% 16%;        /* #2a2a2a equivalent */
  --launcher-bg-control-hover: 217 32% 20%;  /* #333333 equivalent */
  --launcher-border-control: 217 28% 27%;    /* #444 equivalent */

  /* Standardize text hierarchy */
  --launcher-text-primary: 210 40% 96%;      /* Already exists - white text */
  --launcher-text-secondary: 210 20% 62%;    /* Already exists - gray text */
  --launcher-text-muted: 210 13% 40%;        /* Already exists - darker gray */

  /* Add semantic accent colors */
  --launcher-accent-success: 142 100% 66%;   /* #55ff55 (bright green) */
  --launcher-accent-link: 217 100% 50%;      /* #0080ff (blue links) */

  /* Background variations for depth */
  --launcher-bg-elevated: 217 33% 9%;        /* Slightly lighter than primary */
  --launcher-bg-overlay: 217 33% 12%;        /* For dropdowns/modals */
}
```

#### 1.2 Extend Tailwind Theme
**File:** [frontend/src/index.css](frontend/src/index.css) (lines 66-106)

**Add to `@theme inline` block:**
```css
@theme inline {
  /* Add new tokens */
  --color-launcher-bg-control: var(--launcher-bg-control);
  --color-launcher-bg-control-hover: var(--launcher-bg-control-hover);
  --color-launcher-border-control: var(--launcher-border-control);
  --color-launcher-bg-elevated: var(--launcher-bg-elevated);
  --color-launcher-bg-overlay: var(--launcher-bg-overlay);
  --color-launcher-accent-success: var(--launcher-accent-success);
  --color-launcher-accent-link: var(--launcher-accent-link);
}
```

### Phase 2: Refactor Version Selector to Use Theme Tokens

#### 2.1 Update VersionSelector Component
**File:** [frontend/src/components/VersionSelector.tsx](frontend/src/components/VersionSelector.tsx)

**Line 359:** Container background
```tsx
// BEFORE:
className={`w-full h-10 bg-[#2a2a2a] border border-[#444] rounded flex items-center justify-center transition-colors ${

// AFTER:
className={`w-full h-10 bg-[hsl(var(--launcher-bg-control))] border border-[hsl(var(--launcher-border-control))] rounded flex items-center justify-center transition-colors ${
```

**Line 62-66:** Dropdown item states
```tsx
// BEFORE:
className={`relative w-full px-3 py-2 text-left text-sm flex items-center justify-between transition-colors ${
  isActive
    ? 'bg-[#333333] text-[#55ff55]'
    : isInstalling
      ? 'text-gray-500 bg-[#2a2a2a]'
      : 'text-gray-300 hover:bg-[#333333] hover:text-white'

// AFTER:
className={`relative w-full px-3 py-2 text-left text-sm flex items-center justify-between transition-colors ${
  isActive
    ? 'bg-[hsl(var(--launcher-bg-control-hover))] text-[hsl(var(--launcher-accent-success))]'
    : isInstalling
      ? 'text-[hsl(var(--launcher-text-muted))] bg-[hsl(var(--launcher-bg-control))]'
      : 'text-[hsl(var(--launcher-text-secondary))] hover:bg-[hsl(var(--launcher-bg-control-hover))] hover:text-[hsl(var(--launcher-text-primary))]'
```

**Line 96, 129, 378, 423:** Green accent colors
```tsx
// Replace all instances of: text-[#55ff55]
// With: text-[hsl(var(--launcher-accent-success))]

// Replace all instances of: #55ff55
// With: hsl(var(--launcher-accent-success))
```

**Line 129:** Blue link color
```tsx
// BEFORE:
className={isEnabled ? 'text-[#0080ff]' : 'text-gray-500'}

// AFTER:
className={isEnabled ? 'text-[hsl(var(--launcher-accent-link))]' : 'text-[hsl(var(--launcher-text-muted))]'}
```

**Line 371, 443:** Hover backgrounds
```tsx
// Replace all: hover:bg-[#444]
// With: hover:bg-[hsl(var(--launcher-bg-control-hover))]
```

**Line 510:** Dropdown background
```tsx
// BEFORE:
className="absolute top-full left-0 right-0 mt-1 bg-[#2a2a2a] border border-[#444] rounded shadow-lg overflow-hidden z-50"

// AFTER:
className="absolute top-full left-0 right-0 mt-1 bg-[hsl(var(--launcher-bg-overlay))] border border-[hsl(var(--launcher-border-control))] rounded shadow-lg overflow-hidden z-50"
```

#### 2.2 Update Other Hardcoded Colors
Search for remaining hardcoded grays:
- `text-gray-300` → `text-[hsl(var(--launcher-text-secondary))]`
- `text-gray-400` → `text-[hsl(var(--launcher-text-muted))]`
- `text-gray-500` → `text-[hsl(var(--launcher-text-muted))]`
- `text-white` → `text-[hsl(var(--launcher-text-primary))]`

### Phase 3: Standardize StatusFooter Colors

#### 3.1 Update StatusFooter Component
**File:** [frontend/src/components/StatusFooter.tsx](frontend/src/components/StatusFooter.tsx)

Currently uses standard Tailwind colors (blue-400, green-400, orange-400, yellow-400). Consider either:

**Option A (Recommended):** Keep semantic colors as-is since they convey specific meaning
- Blue = downloading/fetching
- Green = success/cached
- Orange = warning
- Yellow = stale

**Option B:** Map to launcher tokens
- Create specific status color tokens if needed
- Example: `--launcher-status-active`, `--launcher-status-warning`, etc.

### Phase 4: Update Remaining Components

#### 4.1 InstallDialog
**File:** [frontend/src/components/InstallDialog.tsx](frontend/src/components/InstallDialog.tsx)

Search for hardcoded hex colors and replace with theme tokens.

#### 4.2 Other Components
Audit all components for hardcoded colors:
```bash
grep -r "#[0-9a-fA-F]\{6\}" frontend/src/components/
grep -r "text-gray" frontend/src/components/
grep -r "bg-gray" frontend/src/components/
```

---

## Improvements to Theme System

### Recommendation 1: Semantic Color Naming
Instead of hardcoding values, use semantic names:

```css
:root {
  /* Surface colors (for backgrounds) */
  --surface-lowest: var(--launcher-bg-primary);
  --surface-low: var(--launcher-bg-secondary);
  --surface-mid: var(--launcher-bg-tertiary);
  --surface-high: var(--launcher-bg-control);
  --surface-highest: var(--launcher-bg-elevated);

  /* Interactive surfaces */
  --surface-interactive: var(--launcher-bg-control);
  --surface-interactive-hover: var(--launcher-bg-control-hover);

  /* Text hierarchy */
  --text-primary: var(--launcher-text-primary);
  --text-secondary: var(--launcher-text-secondary);
  --text-tertiary: var(--launcher-text-muted);

  /* Semantic accents */
  --accent-success: var(--launcher-accent-success);
  --accent-error: var(--launcher-accent-error);
  --accent-info: var(--launcher-accent-info);
  --accent-link: var(--launcher-accent-link);
}
```

**Benefits:**
- Easier to understand component code
- Can swap themes without touching components
- More maintainable

### Recommendation 2: Create Theme Utility Classes

**File:** [frontend/src/index.css](frontend/src/index.css)

```css
@layer components {
  .surface-base {
    @apply bg-[hsl(var(--launcher-bg-primary))];
  }

  .surface-elevated {
    @apply bg-[hsl(var(--launcher-bg-secondary))];
  }

  .surface-control {
    @apply bg-[hsl(var(--launcher-bg-control))]
           border border-[hsl(var(--launcher-border-control))];
  }

  .surface-control-hover {
    @apply hover:bg-[hsl(var(--launcher-bg-control-hover))];
  }

  .text-primary {
    @apply text-[hsl(var(--launcher-text-primary))];
  }

  .text-secondary {
    @apply text-[hsl(var(--launcher-text-secondary))];
  }

  .text-accent-success {
    @apply text-[hsl(var(--launcher-accent-success))];
  }
}
```

**Usage:**
```tsx
// Instead of:
className="bg-[hsl(var(--launcher-bg-control))] border border-[hsl(var(--launcher-border-control))]"

// Use:
className="surface-control"
```

### Recommendation 3: Theme Variants Support

Add support for multiple theme variants (optional future enhancement):

```css
:root {
  /* Default dark theme */
}

:root.theme-darker {
  /* Even darker variant */
  --launcher-bg-primary: 217 40% 3%;
  --launcher-bg-secondary: 217 40% 5%;
}

:root.theme-blue-tint {
  /* More blue saturation */
  --launcher-bg-primary: 220 50% 6%;
}
```

### Recommendation 4: Create Theme Configuration File

**New File:** `frontend/src/config/theme.ts`

```typescript
export const themeColors = {
  surfaces: {
    lowest: 'hsl(var(--launcher-bg-primary))',
    low: 'hsl(var(--launcher-bg-secondary))',
    mid: 'hsl(var(--launcher-bg-tertiary))',
    control: 'hsl(var(--launcher-bg-control))',
    controlHover: 'hsl(var(--launcher-bg-control-hover))',
  },
  text: {
    primary: 'hsl(var(--launcher-text-primary))',
    secondary: 'hsl(var(--launcher-text-secondary))',
    muted: 'hsl(var(--launcher-text-muted))',
  },
  accent: {
    success: 'hsl(var(--launcher-accent-success))',
    error: 'hsl(var(--launcher-accent-error))',
    info: 'hsl(var(--launcher-accent-info))',
    link: 'hsl(var(--launcher-accent-link))',
  },
  borders: {
    default: 'hsl(var(--launcher-border))',
    control: 'hsl(var(--launcher-border-control))',
  },
} as const;
```

**Benefits:**
- Type-safe theme access in TypeScript
- Central source of truth
- Can generate utility classes programmatically

---

## Color Palette Summary

### Proposed Dark Theme Palette
Based on version selector's `#2a2a2a` base:

| Token | HSL Value | Hex Equivalent | Usage |
|-------|-----------|----------------|--------|
| `--launcher-bg-primary` | `217 33% 6%` | `#0a0e12` | Main background |
| `--launcher-bg-secondary` | `217 33% 8%` | `#0d1318` | Elevated sections |
| `--launcher-bg-control` | `217 32% 16%` | `#1f2933` | Inputs, controls |
| `--launcher-bg-control-hover` | `217 32% 20%` | `#2a3441` | Hover states |
| `--launcher-border-control` | `217 28% 27%` | `#3d4955` | Control borders |
| `--launcher-text-primary` | `210 40% 96%` | `#f0f4f8` | Primary text |
| `--launcher-text-secondary` | `210 20% 62%` | `#8a98a8` | Secondary text |
| `--launcher-text-muted` | `210 13% 40%` | `#58646f` | Muted/disabled |
| `--launcher-accent-success` | `142 100% 66%` | `#55ff55` | Success/active |
| `--launcher-accent-link` | `217 100% 50%` | `#0080ff` | Links/info |

---

## Implementation Checklist

- [ ] Phase 1.1: Extend CSS custom properties in index.css
- [ ] Phase 1.2: Add new tokens to Tailwind theme
- [ ] Phase 2.1: Refactor VersionSelector component
- [ ] Phase 2.2: Update hardcoded grays in VersionSelector
- [ ] Phase 3.1: Decide on StatusFooter approach and update
- [ ] Phase 4.1: Update InstallDialog colors
- [ ] Phase 4.2: Audit and update all remaining components
- [ ] Enhancement: Add semantic color naming (optional)
- [ ] Enhancement: Create theme utility classes (recommended)
- [ ] Enhancement: Add theme variants support (optional)
- [ ] Enhancement: Create theme config TypeScript file (optional)
- [ ] Test: Verify consistent appearance across all components
- [ ] Test: Check accessibility (contrast ratios)

---

## Migration Strategy

### Approach 1: Big Bang (Fast)
1. Update all files at once
2. Test thoroughly
3. Ship single commit

**Pros:** Clean git history, immediate consistency
**Cons:** Higher risk, more testing needed upfront

### Approach 2: Incremental (Safe)
1. Add new theme tokens without breaking existing code
2. Migrate components one by one
3. Remove old hardcoded values after all components updated

**Pros:** Lower risk, easier to test, can roll back individual components
**Cons:** Temporary duplication, multiple PRs

**Recommended:** Approach 2 for production, Approach 1 for rapid prototyping

---

## Accessibility Considerations

### Contrast Ratios
Verify WCAG AA compliance (4.5:1 for normal text, 3:1 for large):

- White text (#f0f4f8) on dark backgrounds (#0a0e12): ✅ 16:1
- Secondary text (#8a98a8) on dark: ✅ ~7:1
- Muted text (#58646f) on dark: ⚠️ ~4.6:1 (borderline for small text)
- Green accent (#55ff55): ✅ High contrast on dark

**Recommendation:** Consider slightly brighter muted text if used for important information.

---

## Questions to Consider

1. **StatusFooter colors:** Keep semantic Tailwind colors or unify with theme?
2. **Utility classes:** Create now or wait until more components need theming?
3. **Theme variants:** Should we support light theme or alternative dark themes?
4. **Migration approach:** Big bang or incremental?
5. **Color naming:** Keep "launcher" prefix or use more generic names?

---

## Estimated Effort

- **Phase 1 (CSS tokens):** 15 minutes
- **Phase 2 (VersionSelector):** 30 minutes
- **Phase 3 (StatusFooter):** 15 minutes
- **Phase 4 (Other components):** 1-2 hours
- **Testing:** 30 minutes
- **Enhancements (optional):** 1-2 hours

**Total:** ~3-5 hours for complete implementation with enhancements
