# Coding Standards

## React Aria Usage

### Interaction Hooks

**Always use React Aria hooks for user interactions** instead of native DOM events. React Aria provides robust, accessible, and cross-platform interaction handling.

#### ✅ DO: Use React Aria Hooks

```tsx
import { useHover } from '@react-aria/interactions';

function MyComponent() {
  const { hoverProps, isHovered } = useHover({});

  return (
    <div {...hoverProps}>
      {isHovered ? 'Hovering!' : 'Not hovering'}
    </div>
  );
}
```

#### ❌ DON'T: Use Raw Mouse Events

```tsx
// ❌ Avoid this - unreliable, not accessible
function MyComponent() {
  const [isHovered, setIsHovered] = useState(false);

  return (
    <div
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      {isHovered ? 'Hovering!' : 'Not hovering'}
    </div>
  );
}
```

### Why React Aria?

1. **Reliability**: Handles edge cases like fast mouse movements and window blur
2. **Accessibility**: Properly supports keyboard navigation and screen readers
3. **Cross-platform**: Works correctly on touch devices (doesn't rely on CSS :hover)
4. **Browser consistency**: Normalizes behavior across different browsers

### Available React Aria Hooks

- **useHover**: For hover interactions (replaces onMouseEnter/onMouseLeave)
- **usePress**: For press/click interactions (replaces onClick)
- **useFocus**: For focus interactions (replaces onFocus/onBlur)
- **useKeyboard**: For keyboard interactions (replaces onKeyDown/onKeyUp)

### Enforcement

Our ESLint configuration enforces these standards:

- `onMouseEnter`, `onMouseLeave`, `onMouseOver`, `onMouseOut` are **prohibited**
- Use `npm run lint` to check for violations
- Lint errors will prevent builds in CI/CD

### Resources

- [React Aria Documentation](https://react-spectrum.adobe.com/react-aria/)
- [useHover Hook](https://react-spectrum.adobe.com/react-aria/useHover.html)
- [React Aria Interactions](https://react-spectrum.adobe.com/react-aria/interactions.html)

### Exceptions

In rare cases where React Aria doesn't provide the needed functionality, you may use raw events with:
1. Explicit approval in code review
2. A comment explaining why React Aria can't be used
3. Comprehensive testing for edge cases

```tsx
// eslint-disable-next-line no-restricted-syntax
<div onMouseEnter={handler}>
  {/* Justification: Special case where useHover doesn't apply because... */}
</div>
```
