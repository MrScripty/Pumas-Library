/**
 * Real Chromium layout/interaction regression using the production renderer.
 * The bridge is a fixture: this does not start a backend or migrate metadata.
 * From the repository root, build with npm run -w frontend build, then run
 * with an existing Playwright installation:
 * PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs \
 * CHROMIUM_EXECUTABLE=/absolute/path/to/chromium node frontend/scripts/verify-library-upgrade-viewport.mjs
 * Optionally set PUMAS_VIEWPORT_SCREENSHOT to save the scrolled confirmation.
 */
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { createServer } from 'node:http';
import { fileURLToPath, pathToFileURL } from 'node:url';

assert.ok(process.env.PLAYWRIGHT_MODULE, 'Set PLAYWRIGHT_MODULE; this check installs nothing');
assert.ok(process.env.CHROMIUM_EXECUTABLE, 'Set CHROMIUM_EXECUTABLE to an existing Chromium binary');
const { chromium } = await import(pathToFileURL(process.env.PLAYWRIGHT_MODULE).href);
const build = new URL('../dist/', import.meta.url);
const files = new Map([['/', new URL('index.html', build)]]);
for (const name of await readdir(new URL('assets/', build))) {
  files.set(`/assets/${name}`, new URL(`assets/${name}`, build));
}
const server = createServer(async (request, response) => {
  const pathname = new URL(request.url, 'http://127.0.0.1').pathname;
  const file = files.get(pathname);
  if (!file) {
    response.writeHead(404).end();
    return;
  }
  try {
    const body = await readFile(file);
    const type = pathname.endsWith('.js') ? 'text/javascript'
      : pathname.endsWith('.css') ? 'text/css' : 'text/html';
    response.writeHead(200, { 'Content-Type': type }).end(body);
  } catch {
    response.writeHead(500).end();
  }
});
await new Promise((resolve, reject) => {
  server.once('error', reject);
  server.listen(0, '127.0.0.1', resolve);
});
let browser;
try {
  browser = await chromium.launch({ executablePath: process.env.CHROMIUM_EXECUTABLE, headless: true });
  const page = await browser.newPage({ viewport: { width: 360, height: 400 } });
  page.setDefaultTimeout(5000);
  await page.addInitScript(() => {
    const state = { status: 'recovery-required', reason: 'migration-required',
      authoritySource: 'persisted', action: 'select-library' };
    window.upgradeCalls = [];
    window.electronAPI = {
      get_launcher_root_bootstrap: () => state,
      get_launcher_root_state: async () => state,
      notify_launcher_root_presentation_committed: async () => {},
      onLauncherRootPresentationTimeout: () => () => {},
      minimizeWindow: async () => {},
      close_window: async () => {},
      select_launcher_root: async () => ({ status: 'cancelled' }),
      upgrade_launcher_library: async confirmation => {
        window.upgradeCalls.push(confirmation);
        return { status: 'unavailable' };
      },
    };
  });
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  await page.getByRole('button', { name: 'Upgrade Library', exact: true }).click();
  const heading = page.getByRole('heading', { name: 'Upgrade library metadata', exact: true });
  await heading.waitFor();
  assert.equal(await heading.evaluate(element => document.activeElement === element), true);
  const upgrade = page.getByRole('button', { name: 'Upgrade Metadata and Open Library', exact: true });
  const cancel = page.getByRole('button', { name: 'Cancel', exact: true });
  assert.equal(await upgrade.isDisabled(), true);

  // Actual pointer scrolling must reveal both actions without programmatic focus
  // or Playwright's automatic scroll-into-view rescuing an inaccessible layout.
  await page.mouse.move(180, 300);
  await page.mouse.wheel(0, 600);
  await page.waitForFunction(() => {
    const button = [...document.querySelectorAll('button')].find(element => element.textContent === 'Cancel');
    return button && button.getBoundingClientRect().bottom <= innerHeight;
  });
  for (const button of [upgrade, cancel]) {
    const bounds = await button.boundingBox();
    assert.ok(bounds && bounds.y >= 40 && bounds.y + bounds.height <= 400, 'Action must be fully visible below window controls');
  }
  if (process.env.PUMAS_VIEWPORT_SCREENSHOT) {
    await page.screenshot({ path: process.env.PUMAS_VIEWPORT_SCREENSHOT });
  }
  await cancel.click();
  assert.deepEqual(await page.evaluate(() => window.upgradeCalls), []);

  await page.getByRole('button', { name: 'Upgrade Library', exact: true }).click();
  await heading.waitFor();
  // Tab navigation must scroll each native control into view and retain order.
  for (const label of [/I have closed every other app/, /I understand that older app versions/]) {
    await page.keyboard.press('Tab');
    const checkbox = page.getByRole('checkbox', { name: label });
    assert.equal(await checkbox.evaluate(element => document.activeElement === element), true);
    const bounds = await checkbox.boundingBox();
    assert.ok(bounds && bounds.y >= 40 && bounds.y + bounds.height <= 400);
    await page.keyboard.press('Space');
  }
  assert.equal(await upgrade.isEnabled(), true);
  await page.keyboard.press('Tab');
  assert.equal(await upgrade.evaluate(element => document.activeElement === element), true);
  const bounds = await upgrade.boundingBox();
  assert.ok(bounds && bounds.y >= 40 && bounds.y + bounds.height <= 400);
  await page.keyboard.press('Enter');
  await page.getByRole('heading', { name: 'Library upgrade needs attention', exact: true }).waitFor();
  assert.deepEqual(await page.evaluate(() => window.upgradeCalls), [
    { oldWritersStopped: true, noDowngradeAccepted: true },
  ]);
  console.log(`360x400 pointer scrolling, cancellation, keyboard confirmations and submission passed; Chromium ${browser.version()}; renderer ${fileURLToPath(build)}; simulated bridge only`);
} finally {
  try { await browser?.close(); }
  finally { await new Promise(resolve => server.close(resolve)); }
}
