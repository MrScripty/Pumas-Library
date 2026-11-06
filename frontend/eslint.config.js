import js from '@eslint/js';
import react from 'eslint-plugin-react';
import jsxA11y from 'eslint-plugin-jsx-a11y';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  {
    ignores: ['dist/**', 'node_modules/**', 'scripts/**', '*.config.*'],
  },
  {
    files: ['src/**/*.{ts,tsx}', 'conformance/**/*.test.tsx'],
    extends: [
      js.configs.recommended,
      ...tseslint.configs.strictTypeChecked,
      react.configs.flat.recommended,
      jsxA11y.flatConfigs.recommended,
    ],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      parserOptions: {
        ecmaFeatures: {
          jsx: true,
        },
        project: './tsconfig.json',
      },
    },
    settings: {
      react: {
        version: 'detect',
      },
    },
    rules: {
      // React 19 auto-imports JSX runtime
      'react/react-in-jsx-scope': 'off',
      // TypeScript handles prop validation
      'react/prop-types': 'off',

      // Prevent console usage (must use logger)
      'no-console': 'error',

      // Enforce proper error handling
      '@typescript-eslint/no-floating-promises': 'error',
      '@typescript-eslint/no-explicit-any': 'error',
      // Disabled for now: legacy codebase has broad implicit-return usage.
      '@typescript-eslint/explicit-function-return-type': 'off',
      '@typescript-eslint/no-unused-vars': [
        'error',
        {
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_',
        },
      ],
      // Allow numbers and booleans in template literals
      '@typescript-eslint/restrict-template-expressions': [
        'error',
        { allowNumber: true, allowBoolean: true },
      ],
      // Allow void returns in arrow shorthand
      '@typescript-eslint/no-confusing-void-expression': [
        'error',
        { ignoreArrowShorthand: true },
      ],
      '@typescript-eslint/no-unnecessary-condition': 'error',
      '@typescript-eslint/no-non-null-assertion': 'error',
      // Allow async functions in event handlers and callbacks
      '@typescript-eslint/no-misused-promises': [
        'error',
        { checksVoidReturn: { attributes: false, arguments: false, properties: false } },
      ],
      // Allow {} as a type (commonly used for extensible props)
      '@typescript-eslint/no-empty-object-type': 'off',
      // Allow unknown in catch callbacks (Promise.catch, etc.)
      '@typescript-eslint/use-unknown-in-catch-callback-variable': 'off',
      '@typescript-eslint/no-base-to-string': 'off',
      '@typescript-eslint/no-redundant-type-constituents': 'off',
      // Allow dynamic property deletion (e.g., cleaning up record entries)
      '@typescript-eslint/no-dynamic-delete': 'off',
      // Allow async functions without await (useful for interface conformance)
      '@typescript-eslint/require-await': 'off',
      '@typescript-eslint/no-unnecessary-type-conversion': 'off',

      // Prevent generic Error usage and enforce React Aria patterns
      'no-restricted-syntax': [
        'error',
        {
          selector: 'ThrowStatement > NewExpression[callee.name="Error"]',
          message:
            'Use specific error types from @/errors instead of generic Error',
        },
        // The renderer never opens windows or targets new browsing contexts;
        // external URLs leave through ExternalLink / openExternalUrl.
        {
          selector: 'JSXAttribute[name.name="target"]',
          message:
            'Do not set target on links. Use ExternalLink from components/ui/ExternalLink, which opens the system browser through the desktop bridge.',
        },
        {
          selector: 'MemberExpression[property.name="open"][object.name=/^(window|globalThis|self|top|parent)$/]',
          message:
            'Do not call window.open. Use openExternalUrl from components/ui/openExternalUrl, which opens the system browser through the desktop bridge.',
        },
        // React Aria rules
        {
          selector: 'JSXAttribute[name.name="onMouseEnter"]',
          message:
            'Avoid using onMouseEnter. Use React Aria\'s useHover hook from @react-aria/interactions for robust, accessible hover interactions.',
        },
        {
          selector: 'JSXAttribute[name.name="onMouseLeave"]',
          message:
            'Avoid using onMouseLeave. Use React Aria\'s useHover hook from @react-aria/interactions for robust, accessible hover interactions.',
        },
        {
          selector: 'JSXAttribute[name.name="onMouseOver"]',
          message:
            'Avoid using onMouseOver. Use React Aria\'s useHover hook from @react-aria/interactions for robust, accessible hover interactions.',
        },
        {
          selector: 'JSXAttribute[name.name="onMouseOut"]',
          message:
            'Avoid using onMouseOut. Use React Aria\'s useHover hook from @react-aria/interactions for robust, accessible hover interactions.',
        },
      ],

      // Accessibility rules from jsx-a11y
      'jsx-a11y/mouse-events-have-key-events': 'error',
      'jsx-a11y/no-static-element-interactions': 'error',
    },
  }
);
