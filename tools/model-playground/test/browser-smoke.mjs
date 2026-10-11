// Optional representative-browser evidence. Requires explicitly selected Playwright 1.57.0.
import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import { createPlayground } from '../server.mjs';
import { startFixture, PNG } from './fixture.mjs';

if (!process.env.PLAYWRIGHT_MODULE || !process.env.CHROMIUM_EXECUTABLE) {
  throw new Error('Set PLAYWRIGHT_MODULE to the installed playwright/index.mjs and CHROMIUM_EXECUTABLE to Chromium.');
}
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE);
const fixture = await startFixture();
const app = createPlayground({ backend: fixture.state.endpoint });
await new Promise(resolve => app.server.listen(0, '127.0.0.1', resolve));
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM_EXECUTABLE, headless: true, args: ['--no-sandbox'] });
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1160 } });
  const failures = []; page.on('pageerror', error => failures.push(error.message));
  await page.goto(`http://127.0.0.1:${app.server.address().port}`);
  await page.getByRole('button', { name: 'Discover runtime', exact: true }).focus();
  await page.keyboard.press('Enter');
  await page.waitForFunction(() => document.getElementById('status').textContent.startsWith('Connected.'));
  assert.match(await page.locator('#runtime').textContent(), /SIMULATED/);
  await page.getByRole('button', { name: 'Inspect capabilities', exact: true }).click();
  await page.waitForFunction(() => !document.getElementById('run').disabled);
  assert.equal(await page.getByRole('button', { name: '3D generation', exact: true }).isDisabled(), true);
  assert.equal(await page.getByRole('button', { name: 'Image editing', exact: true }).isDisabled(), true);
  assert.equal(await page.getByRole('button', { name: 'Audio generation', exact: true }).isDisabled(), true);
  const run = async () => {
    await page.getByRole('button', { name: 'Run request', exact: true }).click();
    await page.waitForFunction(() => document.getElementById('status').textContent === 'Received and validated a correlated terminal result.');
  };
  await page.getByRole('button', { name: 'Text', exact: true }).click();
  const original = await page.locator('#request').inputValue();
  await page.locator('#request').fill('{ broken');
  await page.getByRole('button', { name: 'Run request', exact: true }).click();
  assert.equal(fixture.state.operations.length, 0);
  await page.locator('#request').fill(original); await run();
  assert.match(await page.locator('#output').textContent(), /SIMULATED adapter response/);
  await page.getByRole('button', { name: 'Embeddings', exact: true }).click(); await run();
  assert.match(await page.locator('#output').textContent(), /vectors/);
  await page.getByRole('button', { name: 'Image generation', exact: true }).click(); await run();
  await page.locator('#output img').waitFor();
  await page.waitForFunction(() => document.querySelector('#output img').naturalWidth === 1);
  fixture.state.mode = 'invalid-image';
  await page.getByRole('button', { name: 'Run request', exact: true }).click();
  await page.waitForFunction(() => document.getElementById('output').textContent.includes('invalid_image'));
  const invalidImageReceipt = JSON.parse(await page.locator('#receipts').textContent());
  assert.equal(invalidImageReceipt.receipts.at(-1).http_status, 200);
  assert.equal(invalidImageReceipt.error.code, 'invalid_image');
  fixture.state.mode = 'success';
  await page.getByRole('button', { name: 'Image to text', exact: true }).click();
  await page.locator('#file').setInputFiles({ name: 'controlled.png', mimeType: 'image/png', buffer: Buffer.from(PNG, 'base64') });
  await page.waitForFunction(() => document.getElementById('status').textContent.startsWith('Attached')); await run();
  await page.getByRole('button', { name: 'Transcription', exact: true }).click();
  await page.locator('#file').setInputFiles({ name: 'silence.pcm', mimeType: 'application/octet-stream', buffer: Buffer.alloc(32) });
  await page.waitForFunction(() => document.getElementById('status').textContent.startsWith('Attached')); await run();
  await page.getByRole('button', { name: 'Text', exact: true }).click(); await run();
  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Download redacted receipts', exact: true }).click();
  assert.equal((await download).suggestedFilename(), 'pumas-playground-receipt.json');
  await mkdir(new URL('../evidence/', import.meta.url), { recursive: true });
  await page.screenshot({ path: new URL('../evidence/simulated-browser-desktop.png', import.meta.url).pathname, fullPage: true });
  fixture.state.mode = 'hang';
  await page.getByRole('button', { name: 'Run request', exact: true }).click();
  await page.waitForFunction(() => !document.getElementById('cancel').disabled);
  while (!fixture.state.pending.size) await new Promise(resolve => setTimeout(resolve, 5));
  await page.getByRole('button', { name: 'Cancel request', exact: true }).click();
  await page.waitForFunction(() => document.getElementById('output').textContent.includes('cancel_requested'));
  assert.match(await page.locator('#output').textContent(), /unknown/);
  await page.setViewportSize({ width: 390, height: 844 });
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
  await page.screenshot({ path: new URL('../evidence/simulated-browser-mobile.png', import.meta.url).pathname, fullPage: true });
  fixture.state.mode = 'empty';
  await page.getByRole('button', { name: 'Discover runtime', exact: true }).click();
  await page.waitForFunction(() => document.getElementById('status').textContent.includes('no models are served'));
  assert.equal(await page.getByRole('button', { name: 'Run request', exact: true }).isDisabled(), true);
  assert.deepEqual(failures, []);
  process.stdout.write(`PASS: Chromium ${browser.version()}, keyboard discovery, finite inputs/results, attachment, receipt download, cancellation, empty models, 390px layout; SIMULATED inference only.\n`);
} finally { await browser.close(); await app.close(); await fixture.close(); }
