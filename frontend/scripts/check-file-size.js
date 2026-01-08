#!/usr/bin/env node
/**
 * Pre-commit Hook: File Size Checker
 *
 * Validates that all source files remain under the 300-line limit.
 * Helps enforce modular, focused code organization.
 */

import { readFileSync } from 'fs';
import { globSync } from 'glob';

const MAX_LINES = 300;
const violations = [];
const files = globSync('src/**/*.{ts,tsx}', {
  ignore: [
    '**/*.test.{ts,tsx}',
    '**/*.backup.*',
    '**/*.original',
    '**/*.new.*',
    '**/App.new.tsx',
  ],
});

files.forEach((file) => {
  const content = readFileSync(file, 'utf-8');
  const lines = content.split('\n').filter((line) => {
    const trimmed = line.trim();
    return trimmed !== '' && !trimmed.startsWith('//');
  });

  if (lines.length > MAX_LINES) {
    violations.push(`${file}: ${lines.length} lines (max ${MAX_LINES})`);
  }
});

if (violations.length > 0) {
  console.error(`\n❌ File Size Violations Found (>${MAX_LINES} lines):\n`);
  violations.forEach((v) => console.error(`  ${v}`));
  console.error('\nRefactor large files into smaller modules\n');
  process.exit(1);
}

console.log(`✅ File size checks passed (all files <${MAX_LINES} lines)`);
