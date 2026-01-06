import js from '@eslint/js';
import react from 'eslint-plugin-react';
import jsxA11y from 'eslint-plugin-jsx-a11y';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  js.configs.recommended,
  ...tseslint.configs.recommended,
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
        ecmaFeatures: {
          jsx: true,
        },
      },
    },
    settings: {
      react: {
        version: 'detect',
      },
    },
    rules: {
      // Enforce React Aria hooks over raw mouse events
      'no-restricted-syntax': [
        'error',
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
      'jsx-a11y/no-static-element-interactions': 'warn',
    },
  }
);
