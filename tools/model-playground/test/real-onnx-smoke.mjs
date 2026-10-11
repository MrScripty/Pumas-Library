// Opt-in real CPU execution through the public gateway and playground HTTP API.
// Uses Pumas' existing UNTRAINED token-tiling ONNX graph, never a canned response.
import assert from 'node:assert/strict';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdtemp, mkdir, readFile, writeFile, copyFile, rm, realpath } from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { once } from 'node:events';
import { createPlayground } from '../server.mjs';
import { template } from '../contract.mjs';
import { Transport, redact } from '../transport.mjs';

const execute = promisify(execFile);
const repository = fileURLToPath(new URL('../../../', import.meta.url));
const evidenceDirectory = fileURLToPath(new URL('../evidence/', import.meta.url));
const sha256 = async path => {
  const hash = createHash('sha256');
  for await (const bytes of createReadStream(path)) hash.update(bytes);
  return hash.digest('hex');
};
if (!process.env.PUMAS_RPC_BINARY || !process.env.PUMAS_ORT_DIR) {
  throw new Error('Set PUMAS_RPC_BINARY to a built Pumas binary and PUMAS_ORT_DIR to the verified pinned CPU runtime.');
}
const binary = await realpath(process.env.PUMAS_RPC_BINARY);
const runtime = await realpath(process.env.PUMAS_ORT_DIR);
const pinDocument = JSON.parse(await readFile(join(repository, 'scripts/release/onnx-runtime-pins.json')));
const pin = pinDocument.targets['linux-x86_64'];
assert.equal(process.platform, 'linux', 'This smoke owns only the Linux CPU runtime observation.');
for (const [name, file] of Object.entries(pin.files)) assert.equal(await sha256(join(runtime, name)), file.sha256, `Pinned ${name} identity`);

const root = await mkdtemp(join(tmpdir(), 'pumas-playground-onnx-'));
const record = { observed_at: new Date().toISOString(), qualification: 'real_untrained_onnx_execution',
  checkout_commit: (await execute('git', ['rev-parse', 'HEAD'], { cwd: repository })).stdout.trim(),
  public_rust_tree: (await execute('git', ['rev-parse', 'HEAD:rust'], { cwd: repository })).stdout.trim(),
  semantic_model_quality: 'not_qualified', model_download_bytes: 0,
  fixture: 'Pumas synthetic token-tiling ONNX graph; UNTRAINED, not Pantograph BERT',
  binary_sha256: await sha256(binary), runtime_version: pinDocument.version,
  runtime_archive_sha256: pin.archive_sha256, runtime_libraries: pin.files,
  setup: [], requests: [], cleanup: {}, result: null };
let child, childExit, app, browser, modelId, transport, failure, log = '';
const profile = 'playground-onnx-smoke';
const alias = 'untrained-token-tiling-fixture';
const mixedBatch = process.env.PUMAS_SMOKE_MIXED_BATCH === '1';
async function rpc(method, params) {
  const exchange = await transport.exchange('/rpc', { body: { jsonrpc: '2.0', id: `smoke-${method}`, method, params } });
  assert.equal(exchange.status, 200, `${method} HTTP status`);
  assert.equal(exchange.value?.error, undefined, `${method} returned ${JSON.stringify(exchange.value?.error)}`);
  const result = exchange.value.result;
  assert.equal(result.success, true, `${method} failed: ${JSON.stringify(result)}`);
  record.setup.push({ method, success: result.success, loaded: result.loaded, unloaded: result.unloaded, response: redact(result) });
  return result;
}
try {
  const fixture = join(root, 'fixture');
  await execute('python3', [join(repository, 'scripts/release/synthetic_onnx_fixture.py'), '--output-dir', fixture]);
  record.fixture_record = JSON.parse(await readFile(join(fixture, 'synthetic-fixture-record.json')));
  const model = join(root, 'shared-resources/models/embedding/playground/untrained');
  await mkdir(model, { recursive: true });
  for (const name of ['config.json', 'tokenizer.json', 'tokenizer_config.json']) await copyFile(join(fixture, name), join(model, name));
  await copyFile(join(fixture, 'onnx/model_fp16.onnx'), join(model, 'model.onnx'));
  const childEnvironment = { PATH: '/usr/bin:/bin', LANG: 'C.UTF-8', PUMAS_REGISTRY_DB_PATH: join(root, 'registry.db'),
    ORT_DYLIB_PATH: join(runtime, 'libonnxruntime.so'), ORT_SKIP_DOWNLOAD: '1', HF_HUB_OFFLINE: '1', TRANSFORMERS_OFFLINE: '1' };
  for (const name of ['CONFIG', 'DATA', 'CACHE', 'STATE', 'RUNTIME']) {
    const directory = join(root, name.toLowerCase()); await mkdir(directory, { mode: 0o700 });
    childEnvironment[name === 'RUNTIME' ? 'XDG_RUNTIME_DIR' : `XDG_${name}_HOME`] = directory;
  }
  child = spawn(binary, ['--launcher-root', root, '--port', '0'], { cwd: root, env: childEnvironment, stdio: ['ignore', 'pipe', 'pipe'] });
  childExit = once(child, 'exit');
  const receive = bytes => { log = (log + bytes).slice(-65536); };
  child.stdout.on('data', receive); child.stderr.on('data', receive);
  const started = Date.now(); let match;
  while (!(match = /RPC_PORT=(\d+)/.exec(log))) {
    if (child.exitCode !== null || Date.now() - started > 45000) throw new Error('Pumas did not publish a port within the startup budget.');
    await new Promise(resolve => setTimeout(resolve, 25));
  }
  transport = new Transport(`http://127.0.0.1:${match[1]}`);
  const imported = await rpc('import_model_in_place', { model_dir: model, official_name: 'UNTRAINED token-tiling fixture', family: 'nomic_bert', model_type: 'embedding' });
  modelId = imported.model_id;
  await rpc('upsert_runtime_profile', { profile: { profile_id: profile, provider: 'onnx_runtime', provider_mode: 'onnx_serve', management_mode: 'managed', name: 'Playground UNTRAINED CPU smoke' } });
  await rpc('launch_runtime_profile', { profile_id: profile });
  const served = await rpc('serve_model', { request: { model_id: modelId, config: { provider: 'onnx_runtime', profile_id: profile, device_mode: 'auto', keep_loaded: true, model_alias: alias } } });
  assert.equal(served.loaded, true, `Real ONNX session must be loaded: ${JSON.stringify(redact(served))}`);
  assert.ok((await readFile(`/proc/${child.pid}/maps`, 'utf8')).includes(join(runtime, 'libonnxruntime.so')), 'Selected pinned runtime must actually be mapped.');
  record.runtime_mapping = 'selected pinned library observed in real Pumas process';

  app = createPlayground({ backend: transport.endpoint });
  await new Promise(resolve => app.server.listen(0, '127.0.0.1', resolve));
  const origin = `http://127.0.0.1:${app.server.address().port}`;
  const html = await (await fetch(origin)).text();
  const session = /name="playground-session" content="([^"]+)"/.exec(html)[1];
  const post = async (path, input) => {
    const response = await fetch(origin + path, { method: 'POST', headers: { 'content-type': 'application/json', 'x-playground-session': session }, body: JSON.stringify(input) });
    const value = await response.json(); record.requests.push(...value.receipts ?? []);
    assert.equal(value.error, undefined, `${path}: ${JSON.stringify(value.error)}`);
    return value;
  };
  const discovery = await post('/api/discover', { id: 'onnx-smoke-discovery' });
  assert.ok(discovery.models.some(item => item.id === alias)); record.build = discovery.build; record.fence = discovery.fence;
  const inspected = await post('/api/inspect', { id: 'onnx-smoke-inspect', model: alias, profile, fence: discovery.fence });
  assert.equal(inspected.rows.find(row => row.capability === 'text_embedding').support, 'supported');
  const request = template('text_embedding', alias, profile, 'onnx-smoke-real-request');
  request.input = mixedBatch ? { kind: 'text_batch', texts: ['hello world', 'world'] } : { kind: 'text', text: 'hello world' };
  request.options.dimensions = 256;
  const outcome = await post('/api/run', { id: 'onnx-smoke-run', request, fence: discovery.fence });
  assert.equal(outcome.state, 'succeeded');
  const vectors = outcome.result.vectors;
  assert.equal(vectors.length, mixedBatch ? 2 : 1);
  // The source graph tiles each positive token value 256 times. Mean pooling
  // and L2 normalization yield 1/sqrt(256), independently of this HTTP client.
  assert.ok(vectors.every(vector => vector.length === 256 && vector.every(value => Number.isFinite(value) && Math.abs(value - 1 / Math.sqrt(256)) < 1e-6)));
  record.result = { state: 'succeeded', actual_vectors: vectors, expected_component: 0.0625,
    oracle: 'Source graph token-tiling + mean pooling + L2 normalization', max_absolute_error: Math.max(...vectors.flat().map(v => Math.abs(v - 0.0625))) };
  if (process.env.PLAYWRIGHT_MODULE && process.env.CHROMIUM_EXECUTABLE) {
    const { chromium } = await import(process.env.PLAYWRIGHT_MODULE);
    browser = await chromium.launch({ executablePath: process.env.CHROMIUM_EXECUTABLE, headless: true, args: ['--no-sandbox'] });
    const page = await browser.newPage({ viewport: { width: 1440, height: 1160 } });
    await page.goto(origin);
    await page.getByRole('button', { name: 'Discover runtime', exact: true }).click();
    await page.waitForFunction(() => !document.getElementById('inspect').disabled);
    await page.getByRole('button', { name: 'Inspect capabilities', exact: true }).click();
    await page.waitForFunction(() => !document.getElementById('run').disabled);
    const browserRequest = JSON.parse(await page.locator('#request').inputValue());
    assert.equal(browserRequest.capability, 'text_embedding');
    browserRequest.input = { kind: 'text', text: 'hello world' }; browserRequest.options.dimensions = 256;
    await page.locator('#request').fill(JSON.stringify(browserRequest, null, 2));
    await page.getByRole('button', { name: 'Run request', exact: true }).click();
    await page.waitForFunction(() => document.getElementById('status').textContent === 'Received and validated a correlated terminal result.');
    const rendered = JSON.parse(await page.locator('#output').textContent());
    assert.ok(rendered.vectors[0].length === 256 && rendered.vectors[0].every(v => Math.abs(v - 0.0625) < 1e-6));
    record.browser_receipt = JSON.parse(await page.locator('#receipts').textContent());
    record.browser = { version: browser.version(), real_ui_request: 'passed', visible_result: 'passed' };
    await page.screenshot({ path: join(evidenceDirectory, 'real-untrained-onnx-browser.png'), fullPage: true });
  }
} catch (error) {
  failure = error; record.failure = { message: error.message };
  record.runtime_log_excerpt = redact(log);
} finally {
  if (browser) await browser.close();
  if (app) await app.close();
  if (modelId && transport) {
    try {
      const result = await rpc('unserve_model', { request: { model_id: modelId, provider: 'onnx_runtime', profile_id: profile, model_alias: alias } });
      assert.equal(result.unloaded, true); record.cleanup.unload = 'passed';
    } catch (error) { record.cleanup.unload = 'failed'; failure ??= error; }
  }
  if (child) {
    if (child.exitCode === null) child.kill('SIGINT');
    let timer;
    await Promise.race([childExit, new Promise(resolve => { timer = setTimeout(() => { record.cleanup.forced_stop = true; child.kill('SIGKILL'); resolve(); }, 20000); })]);
    clearTimeout(timer); if (child.exitCode === null && child.signalCode === null) await childExit;
    record.cleanup.exit_code = child.exitCode;
    if (record.cleanup.forced_stop || child.exitCode !== 0) failure ??= new Error('Runtime did not shut down cleanly.');
  }
  await mkdir(evidenceDirectory, { recursive: true });
  record.qualification = failure ? 'failed_real_untrained_onnx_attempt' : 'real_untrained_onnx_execution';
  await writeFile(join(evidenceDirectory, mixedBatch ? 'real-untrained-onnx-batch-failure.json' : 'real-untrained-onnx.json'), JSON.stringify(record, null, 2) + '\n');
  await rm(root, { recursive: true, force: true });
}
if (failure) throw failure;
process.stdout.write('PASS: real UNTRAINED ONNX CPU execution through playground; 256-dimensional output matches the graph oracle, unloaded and shut down cleanly.\n');
