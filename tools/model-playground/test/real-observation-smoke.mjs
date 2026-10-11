// Opt-in: starts only the explicitly selected binary, with an empty disposable root.
// No inference, model loading, acquisition or existing Pumas instance is contacted.
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { once } from 'node:events';
import { PumasClient } from '../client.mjs';
import { Transport } from '../transport.mjs';
import { createPlayground } from '../server.mjs';

if (!process.env.PUMAS_RPC_BINARY) throw new Error('Set PUMAS_RPC_BINARY to an existing authorized local binary.');
const binary = process.env.PUMAS_RPC_BINARY;
const root = await mkdtemp(join(tmpdir(), 'pumas-playground-smoke-'));
const child = spawn(binary, ['--launcher-root', root, '--port', '0'], {
  env: { ...process.env, PUMAS_REGISTRY_DB_PATH: join(root, 'registry.db') }, stdio: ['ignore', 'pipe', 'pipe'],
});
let stdout = '', stderr = ''; child.stdout.on('data', b => { stdout = (stdout + b).slice(-65536); });
child.stderr.on('data', b => { stderr = (stderr + b).slice(-65536); });
const exit = once(child, 'exit');
try {
  const start = Date.now(); let match;
  while (!(match = /RPC_PORT=(\d+)/.exec(stdout))) {
    if (child.exitCode !== null || Date.now() - start > 30000) throw new Error('Selected binary did not publish a port within the smoke startup budget.');
    await new Promise(resolve => setTimeout(resolve, 25));
  }
  const transport = new Transport(`http://127.0.0.1:${match[1]}`);
  const health = await transport.exchange('/health');
  if (health.value?.status !== 'ok') throw new Error('Existing runtime health failed.');
  const receipts = []; let result;
  try {
    const observation = await new PumasClient(transport).discover(new AbortController().signal, receipts);
    if (observation.models.length !== 0) throw new Error('Disposable library unexpectedly has served models.');
    result = { state: 'compatible_empty_runtime', inference: 'untested' };
  } catch (error) {
    if (error.code !== 'unsupported_contract') throw error;
    result = { state: 'unavailable', code: error.code, message: error.message, inference: 'untested' };
  }
  const evidence = { observed_at: new Date().toISOString(), kind: 'real_existing_binary_empty_library',
    binary_sha256: createHash('sha256').update(await readFile(binary)).digest('hex'),
    model_execution: 'none', health: health.value, result, receipts };
  await writeFile(new URL('../evidence/real-runtime-observation.json', import.meta.url), JSON.stringify(evidence, null, 2) + '\n');
  if (process.env.PLAYWRIGHT_MODULE && process.env.CHROMIUM_EXECUTABLE) {
    const { chromium } = await import(process.env.PLAYWRIGHT_MODULE);
    const app = createPlayground({ backend: transport.endpoint });
    await new Promise(resolve => app.server.listen(0, '127.0.0.1', resolve));
    const browser = await chromium.launch({ executablePath: process.env.CHROMIUM_EXECUTABLE, headless: true, args: ['--no-sandbox'] });
    try {
      const page = await browser.newPage({ viewport: { width: 1440, height: 1100 } });
      await page.goto(`http://127.0.0.1:${app.server.address().port}`);
      await page.getByRole('button', { name: 'Discover runtime', exact: true }).click();
      await page.waitForFunction(() => !document.getElementById('discover').disabled);
      await page.screenshot({ path: new URL('../evidence/real-runtime-browser.png', import.meta.url).pathname, fullPage: true });
    } finally { await browser.close(); await app.close(); }
  }
  process.stdout.write(`PASS: real runtime health and ${result.state}; no inference or downloads.\n`);
} finally {
  if (child.exitCode === null) child.kill('SIGTERM');
  let timer;
  await Promise.race([exit, new Promise(resolve => { timer = setTimeout(() => { child.kill('SIGKILL'); resolve(); }, 10000); })]);
  clearTimeout(timer); if (child.exitCode === null && child.signalCode === null) await exit;
  await rm(root, { recursive: true, force: true });
}
