#!/usr/bin/env node
/**
 * Pre-commit Hook: Error Handling Checker
 *
 * Validates that all error handling follows the coding standards:
 * - No console.* usage (use logger instead)
 * - Catch blocks use type guards (instanceof)
 * - No throw new Error() (use specific error types)
 */

import { readFileSync } from 'fs';
import { globSync } from 'glob';

const violations = [];
const files = globSync('src/**/*.{ts,tsx}', {
  ignore: [
    '**/*.test.{ts,tsx}',
    '**/*.backup.*',
    '**/*.original',
    '**/*.new.*',
    '**/App.new.tsx',
    '**/*.phase3-backup',
  ],
});

files.forEach((file) => {
  const content = readFileSync(file, 'utf-8');
  const lines = content.split('\n');

  lines.forEach((line, index) => {
    const lineNum = index + 1;

    // Check for console.log/error/warn/info/debug (unless noqa comment)
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
    if (/throw new Error\(/.test(line)) {
      const hasNoqaOnLine = /noqa:.*generic-error/.test(line);
      const hasNoqaOnPrevLine = index > 0 && /noqa:.*generic-error/.test(lines[index - 1]);

      if (!hasNoqaOnLine && !hasNoqaOnPrevLine) {
        violations.push(
          `${file}:${lineNum} - Use specific error types from @/errors instead of generic Error`
        );
      }
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
