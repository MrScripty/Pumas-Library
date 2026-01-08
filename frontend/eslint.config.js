import js from '@eslint/js';
import react from 'eslint-plugin-react';
import jsxA11y from 'eslint-plugin-jsx-a11y';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  js.configs.recommended,
  ...tseslint.configs.strictTypeChecked,
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
        project: './tsconfig.json',
      },
    },
    settings: {
      react: {
        version: 'detect',
      },
    },
    rules: {
      // Prevent console usage (must use logger)
      'no-console': 'error',

      // Enforce proper error handling
      '@typescript-eslint/no-floating-promises': 'error',
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/explicit-function-return-type': [
        'warn',
        {
          allowExpressions: true,
          allowTypedFunctionExpressions: true,
        },
      ],
      '@typescript-eslint/no-unused-vars': [
        'error',
        {
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_',
        },
      ],

      // File size and complexity limits
      'max-lines': [
        'warn',
        {
          max: 300,
          skipBlankLines: true,
          skipComments: true,
        },
      ],
      'max-lines-per-function': [
        'warn',
        {
          max: 50,
          skipBlankLines: true,
          skipComments: true,
        },
      ],
      complexity: ['warn', 15],

      // Prevent generic Error usage and enforce type guards
      'no-restricted-syntax': [
        'error',
        {
          selector: 'ThrowStatement > NewExpression[callee.name="Error"]',
          message:
            'Use specific error types from @/errors instead of generic Error',
        },
        {
          selector: 'CatchClause > Identifier[name="error"]:not([typeAnnotation])',
          message:
            'Catch clauses should use type guards (if (error instanceof ...))',
        },
        // Existing React Aria rules
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
