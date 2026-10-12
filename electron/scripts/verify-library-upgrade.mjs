/** Native Linux release acceptance: metadata-only migration and the real recovery UI. */
import assert from 'node:assert/strict';
import console from 'node:console';
import { spawn, spawnSync } from 'node:child_process';
import { chmodSync, closeSync, ftruncateSync, mkdirSync, mkdtempSync, openSync, readFileSync,
  readdirSync, statSync, writeFileSync, writeSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { clearTimeout, setTimeout } from 'node:timers';
import { setTimeout as delay } from 'node:timers/promises';
import { URL } from 'node:url';

const { WebSocket, fetch } = globalThis;

const [executable, rpc] = process.argv.slice(2);
assert.ok(executable && rpc, 'Provide extracted desktop executable and packaged RPC binary');
assert.equal(process.platform, 'linux', 'This acceptance harness requires native Linux');
const base = mkdtempSync(join(tmpdir(), 'pumas-metadata-upgrade-'));
chmodSync(base, 0o700);
const legacy = Buffer.from(JSON.stringify({ schema_version: 5, downloads: [], recovery_revocations: {},
  lifecycle_quarantines: {}, admission_attempts: {}, queue_admissions: {}, released_queue_admissions: {} }));
function fixture(name) {
  const root = join(base, name);
  mkdirSync(join(root, 'launcher-data'), { recursive: true });
  mkdirSync(join(root, 'shared-resources/models'), { recursive: true });
  writeFileSync(join(root, 'launcher-data/downloads.json'), legacy, { mode: 0o600 });
  const config = join(base, `${name}-config`);
  mkdirSync(config);
  const env = { ...process.env, XDG_CONFIG_HOME: config, PUMAS_CONFIG_DIR: config, APPDATA: config };
  for (const key of ['PUMAS_LAUNCHER_ROOT', 'PUMAS_RPC_BINARY', 'ELECTRON_RUN_AS_NODE', 'PUMAS_RELEASE_SMOKE']) delete env[key];
  return { root, config, env };
}
function backups(root) {
  return readdirSync(join(root, 'launcher-data')).filter(name => name.startsWith('downloads.pre-schema7-'));
}
function verifyMetadata(root, original = legacy) {
  assert.equal(backups(root).length, 1);
  const backup = join(root, 'launcher-data', backups(root)[0]);
  assert.deepEqual(readFileSync(backup), original);
  assert.equal(statSync(backup).mode & 0o777, 0o600);
  const upgraded = JSON.parse(readFileSync(join(root, 'launcher-data/downloads.json')));
  assert.equal(upgraded.schema_version, 7);
  for (const [key, value] of Object.entries(JSON.parse(original))) {
    if (key !== 'schema_version') assert.deepEqual(upgraded[key], value);
  }
}
async function until(check, timeout = 30000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) { const result = await check(); if (result) return result; await delay(50); }
  throw new Error('Packaged upgrade observation timed out');
}
async function connect(url) {
  const socket = new WebSocket(url);
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true });
    socket.addEventListener('error', reject, { once: true });
  });
  let id = 0;
  const pending = new Map();
  socket.addEventListener('message', ({ data }) => {
    const message = JSON.parse(data);
    const receipt = pending.get(message.id);
    if (!receipt) return;
    pending.delete(message.id); clearTimeout(receipt.timer);
    if (message.error) receipt.reject(new Error(message.error.message)); else receipt.resolve(message.result);
  });
  return { socket, evaluate(expression) {
    return new Promise((resolve, reject) => {
      const requestId = ++id;
      const timer = setTimeout(() => { pending.delete(requestId); reject(new Error('CDP evaluation timed out')); }, 10000);
      pending.set(requestId, { timer, reject, resolve: receipt => {
        if (receipt.exceptionDetails) reject(new Error('Packaged renderer evaluation failed'));
        else resolve(receipt.result.value);
      } });
      socket.send(JSON.stringify({ id: requestId, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }));
    });
  } };
}

// Four tebibytes of logical model data, with only the sentinel blocks allocated.
// Invoke the exact production helper without starting the model indexer afterward.
const huge = fixture('huge-model');
const weights = join(huge.root, 'shared-resources/models/weights.gguf');
const modelSize = 4 * 1024 ** 4;
const file = openSync(weights, 'wx', 0o600);
try { ftruncateSync(file, modelSize); writeSync(file, Buffer.from('retained weights'), 0, 16, 0); }
finally { closeSync(file); }
const before = statSync(weights, { bigint: true });
assert.ok(before.blocks * 512n < 1024n * 1024n, 'The terabyte fixture must be sparse');
const began = Date.now();
const receipt = spawnSync(rpc, ['--migrate-download-store-offline', '--launcher-root', huge.root,
  '--confirm-old-writers-stopped'], { env: huge.env, timeout: 15000, encoding: 'utf8' });
assert.equal(receipt.status, 0, receipt.stderr);
verifyMetadata(huge.root);
const after = statSync(weights, { bigint: true });
for (const key of ['ino', 'size', 'blocks', 'atimeNs', 'mtimeNs', 'ctimeNs']) assert.equal(after[key], before[key], key);
assert.deepEqual(readdirSync(join(huge.root, 'shared-resources/models')), ['weights.gguf']);
for (const name of readdirSync(huge.root, { recursive: true })) {
  const info = statSync(join(huge.root, name));
  if (info.isFile() && info.size > 1024 * 1024) assert.equal(join(huge.root, name), weights, 'No model backup may be created');
}
console.log(`Metadata-only CLI: 4 TiB model unchanged; exact private JSON backup; ${Date.now() - began} ms`);

const gui = fixture('desktop');
// A retained failed download may point at a folder that was never created.
// This caught a real failure hidden by an empty download-queue fixture.
const libraryId = 'b46c40d5-a5e6-4eb9-8b94-f03780385692';
const downloadId = '31bf70a5-e93c-4735-9de4-01f3feecb065';
const attemptId = '9e3aa464-12e3-455d-8e21-00766bf0d16f';
const missingModel = join(gui.root, 'shared-resources/models/missing-model');
writeFileSync(join(gui.root, 'shared-resources/models/.pumas-library-id.json'),
  JSON.stringify({ schema_version: 1, library_id: libraryId }));
const retainedLegacy = Buffer.from(JSON.stringify({ ...JSON.parse(legacy), downloads: [{
  download_id: downloadId, repo_id: 'acme/model', filename: 'weights.gguf', filenames: ['weights.gguf'],
  dest_dir: missingModel, total_bytes: 8, status: 'error', revision: null, created_at: '2026-10-10T00:00:00Z',
  known_sha256: null, huggingface_evidence: null,
  download_request: { repo_id: 'acme/model', family: 'acme', official_name: 'Model', model_type: 'llm',
    quant: null, filename: 'weights.gguf', filenames: ['weights.gguf'], pipeline_tag: null,
    bundle_format: null, pipeline_class: null, release_date: null, download_url: null,
    model_card_json: null, license_status: null },
}], queue_admissions: { [downloadId]: {
  attempt_id: attemptId, destination: { library_root: `uuid:${libraryId}`, relative_target: 'missing-model' },
  domain: 'ambient', execution_files: ['weights.gguf'], requested_payload_files: ['weights.gguf'],
  position: { ordinal: 0, predecessor: null },
} } }));
writeFileSync(join(gui.root, 'launcher-data/downloads.json'), retainedLegacy, { mode: 0o600 });
const userData = join(gui.config, 'pumas-library-electron');
mkdirSync(userData);
writeFileSync(join(userData, 'launcher-root.json'), JSON.stringify({ launcherRoot: gui.root, selectedPath: gui.root,
  updatedAt: new Date().toISOString() }));
async function verifyDesktop(reopening = false) {
  const app = spawn(executable, ['--disable-gpu', '--disable-dev-shm-usage', '--remote-debugging-port=0'],
    { cwd: dirname(executable), env: gui.env, detached: true, stdio: ['ignore', 'pipe', 'pipe'] });
  let output = '';
  const record = chunk => { output = (output + chunk).slice(-100000); };
  app.stdout.on('data', record); app.stderr.on('data', record);
  const exited = new Promise(resolve => app.once('close', (code, signal) => resolve({ code, signal })));
  let cdp;
  try {
    const browser = await until(() => {
      if (app.exitCode !== null || app.signalCode !== null) throw new Error('Desktop exited before upgrade');
      return output.match(/DevTools listening on (ws:\/\/127\.0\.0\.1:\d+\/\S+)/)?.[1];
    });
    const origin = new URL(browser).origin.replace('ws:', 'http:');
    const page = await until(async () => (await (await fetch(`${origin}/json/list`)).json()).find(target => target.type === 'page'));
    cdp = await connect(page.webSocketDebuggerUrl);
    if (!reopening) {
      await until(async () => (await cdp.evaluate("document.querySelector('h1')?.textContent")) === 'Library upgrade required');
      const click = name => cdp.evaluate(`(() => { const button = [...document.querySelectorAll('button')].find(b => b.textContent === ${JSON.stringify(name)}); if (!button || button.disabled) throw new Error('Unavailable button'); button.click(); })()`);
      await click('Upgrade Library');
      await until(async () => (await cdp.evaluate("document.querySelector('h1')?.textContent")) === 'Upgrade library metadata');
      assert.equal(await cdp.evaluate("document.body.textContent.includes('Model files are not copied')"), true);
      assert.equal(await cdp.evaluate("[...document.querySelectorAll('button')].find(b => b.textContent === 'Upgrade Metadata and Open Library')?.disabled"), true);
      await click('Cancel');
      await until(async () => (await cdp.evaluate("document.querySelector('h1')?.textContent")) === 'Library upgrade required');
      assert.deepEqual(readFileSync(join(gui.root, 'launcher-data/downloads.json')), retainedLegacy);
      assert.equal(backups(gui.root).length, 0);
      await click('Upgrade Library');
      await until(async () => (await cdp.evaluate("document.querySelectorAll('input[type=checkbox]').length")) === 2);
      await cdp.evaluate("document.querySelectorAll('input[type=checkbox]')[0].click()");
      await delay(50);
      assert.equal(await cdp.evaluate("[...document.querySelectorAll('button')].find(b => b.textContent === 'Upgrade Metadata and Open Library')?.disabled"), true);
      await cdp.evaluate("document.querySelectorAll('input[type=checkbox]')[1].click()");
      await until(async () => (await cdp.evaluate("[...document.querySelectorAll('button')].find(b => b.textContent === 'Upgrade Metadata and Open Library')?.disabled")) === false);
      await click('Upgrade Metadata and Open Library');
    }
    await until(async () => (await cdp.evaluate('window.electronAPI.get_launcher_root_state()'))?.status === 'ready');
    await until(async () => (await cdp.evaluate("document.querySelector('#launcher-root-recovery-title') === null")) === true);
    verifyMetadata(gui.root, retainedLegacy);
    assert.equal(readdirSync(join(gui.root, 'shared-resources/models')).includes('missing-model'), false);
    assert.equal(app.exitCode, null, 'Upgrade must open the library in the same desktop session');
    await Promise.race([cdp.evaluate('window.electronAPI.close_window()'), exited]);
    const closed = await Promise.race([exited, delay(15000).then(() => { throw new Error('Healthy upgraded desktop failed to close'); })]);
    assert.equal(closed.signal, null); assert.equal(closed.code, 0);
    console.log(reopening ? 'Native packaged reopen: retained download preserved; backend ready; clean close; no stale instance claim'
      : 'Native packaged UI: cancellation unchanged; both confirmations required; exact metadata backup; retained missing-folder download preserved; backend ready; same window opens library; clean close');
  } catch (error) {
    writeFileSync(join(base, 'desktop-failure.log'), output, { mode: 0o600 });
    throw error;
  } finally {
    cdp?.socket.close();
    if (app.exitCode === null && app.signalCode === null) {
      process.kill(-app.pid, 'SIGTERM'); await Promise.race([exited, delay(5000)]);
    }
    console.log(`Upgrade evidence retained at ${base}`);
  }
}

await verifyDesktop();
await verifyDesktop(true);
