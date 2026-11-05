#!/usr/bin/env node
// Checks a built renderer bundle against its variant:
//
//   node scripts/release/check-frontend-variant.mjs DIR library-only|default
//
// The library-only variant (`vite build --mode library-only`) replaces the
// inference-plugin modules with stubs (frontend/vite.config.ts). Each marker is
// a string literal that exists only in one replaced module, so it survives
// minification in the default bundle and must be absent from library-only.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const INFERENCE_ONLY_MARKERS = [
  { source: 'frontend/src/components/ModelRuntimeRouteEditor.tsx', text: 'Select a runtime profile before saving route' },
  { source: 'frontend/src/components/ModelRuntimeRouteEditor.tsx', text: 'Select a runtime profile before clearing route' },
];

function scripts(directory) {
  const index = path.join(directory, 'index.html');
  if (!fs.existsSync(index)) throw new Error(`${directory}: no index.html`);
  const html = fs.readFileSync(index, 'utf8');
  const referenced = [...html.matchAll(/<script[^>]+src="\.\/([^"]+\.js)"/g)].map(match => match[1]);
  if (referenced.length === 0) throw new Error(`${directory}: index.html references no relative script`);
  for (const file of referenced) {
    if (!fs.existsSync(path.join(directory, file))) throw new Error(`${directory}: index.html references missing ${file}`);
  }
  const files = fs.readdirSync(directory, { recursive: true, withFileTypes: true })
    .filter(entry => entry.isFile() && entry.name.endsWith('.js'))
    .map(entry => path.join(entry.parentPath, entry.name));
  return files.map(file => fs.readFileSync(file, 'utf8')).join('\n');
}

export function checkFrontendVariant(directory, variant) {
  if (!['library-only', 'default'].includes(variant)) throw new Error(`Unknown frontend variant: ${variant}`);
  const bundle = scripts(directory);
  for (const marker of INFERENCE_ONLY_MARKERS) {
    const present = bundle.includes(marker.text);
    if (variant === 'library-only' && present) {
      throw new Error(`library-only bundle contains inference-only code from ${marker.source}: "${marker.text}"`);
    }
    if (variant === 'default' && !present) {
      throw new Error(`default bundle lacks inference-plugin code from ${marker.source}: "${marker.text}"`);
    }
  }
  return INFERENCE_ONLY_MARKERS.length;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [directory, variant] = process.argv.slice(2);
  try {
    const count = checkFrontendVariant(directory, variant);
    console.log(`Verified ${variant} renderer in ${directory} against ${count} inference-only markers`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
