# frontend components ui

## Purpose
Small reusable UI primitives used across feature components.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `IconButton.tsx` | Button primitive with icon-focused affordances. |
| `Tooltip.tsx` | Tooltip helper component. |
| `ListItem.tsx` | Shared list row wrapper. |
| `EmptyState.tsx` | Empty-state presentation component. |
| `HoldToDeleteButton.tsx` | Destructive-action button with hold confirmation. |

## Design Decisions
- Keep primitives presentational and stateless where possible.
- Feature-specific behavior should stay outside this directory.

## Dependencies
**Internal:** shared styling conventions.
**External:** React.

## Usage Examples
```tsx
<IconButton label="Refresh" onClick={refresh} />
```
