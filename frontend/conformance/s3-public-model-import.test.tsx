/** Real UI -> built preload -> privileged validators -> local RPC -> HTTPS S3.
 * Tiny actual format bytes qualify registration; they are not inference fixtures. */
import { spawn, spawnSync, type ChildProcess } from 'node:child_process';
import { createHash } from 'node:crypto';
import { once } from 'node:events';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createServer } from 'node:https';
import { tmpdir } from 'node:os';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { runInNewContext } from 'node:vm';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ValidationError } from '../src/errors';
import { S3ModelImportDialog } from '../src/components/S3ModelImportDialog';
import { decodeS3PersistedImportsWire } from '../src/generated/desktop-contract';
// Load the built privileged boundary, avoiding a second UI-side implementation.
const require = createRequire(import.meta.url);
const { validateApiCallPayload } = require(resolve('../electron/dist/ipc-validation.js')) as {
  validateApiCallPayload: (method: unknown, params: unknown) => { method: string; params: Record<string, unknown> };
};
const { receiveS3ImportRpc } = require(resolve('../electron/dist/s3-import-rpc.js')) as {
  receiveS3ImportRpc: (method: string, invoke: () => Promise<unknown>) => Promise<unknown>;
};

function candidateBinary(): string {
  const configured = process.env['PUMAS_S3_PUBLIC_RPC_BIN'];
  if (configured) return configured;
  // The ordinary conformance command must exercise this gate without requiring
  // an unconfigured CI environment variable or substituting a mock producer.
  const built = spawnSync('cargo', [
    'build', '--locked', '--offline', '--manifest-path', resolve('../rust/Cargo.toml'),
    '-p', 'pumas-rpc', '--no-default-features', '--features', 's3', '--message-format=json',
  ], { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });
  if (built.error || built.status !== 0) {
    throw new ValidationError(`Native S3 RPC build failed: ${built.error?.message ?? built.stderr}`, 'rpc-fixture');
  }
  for (const line of built.stdout.split('\n').filter(Boolean)) {
    const event = record(JSON.parse(line));
    if (event['reason'] === 'compiler-artifact' && typeof event['executable'] === 'string'
        && record(event['target'])['name'] === 'pumas-rpc') return event['executable'];
  }
  throw new ValidationError('Cargo did not report the native S3 RPC executable.', 'rpc-fixture');
}
const binary = candidateBinary();
const tls = resolve('../rust/crates/pumas-core/tests/fixtures/http-tls');
const preload = readFileSync(resolve('../electron/dist/preload.js'), 'utf8');
type Files = Map<string, Buffer>;
function record(value: unknown): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) throw new ValidationError('Expected fixture object.', 'fixture');
  return value as Record<string, unknown>;
}
function parsed(bytes: Buffer): Record<string, unknown> { const value: unknown = JSON.parse(bytes.toString()); return record(value); }
function bytes(files: Files, path: string): Buffer {
  const value = files.get(path); if (!value) throw new ValidationError(`Missing fixture bytes: ${path}`, 'fixture'); return value;
}
async function within<T>(promise: Promise<T>, milliseconds: number): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try { return await Promise.race([promise, new Promise<never>((_, fail) => {
    timer = setTimeout(() => fail(new ValidationError('Fixture operation deadline.', 'fixture')), milliseconds);
  })]); } finally { clearTimeout(timer); }
}
const json = (value: unknown) => Buffer.from(JSON.stringify(value));
const digest = (bytes: Buffer) => createHash('sha256').update(bytes).digest('hex');
function tensor(name: string): Buffer {
  let header = JSON.stringify({ [name]: { dtype: 'F32', shape: [1], data_offsets: [0, 4] } });
  while (Buffer.byteLength(header) % 8 !== 0) header += ' ';
  const size = Buffer.alloc(8); size.writeBigUInt64LE(BigInt(Buffer.byteLength(header)));
  const data = Buffer.alloc(4); data.writeFloatLE(1);
  return Buffer.concat([size, Buffer.from(header), data]);
}
function gguf(): Buffer {
  const bytes = Buffer.alloc(24); bytes.write('GGUF'); bytes.writeUInt32LE(3, 4); return bytes;
}
const tokenizer = () => json({ version: '1.0', truncation: null, padding: null,
  added_tokens: [], normalizer: null, pre_tokenizer: null, post_processor: null,
  decoder: null, model: { type: 'WordLevel', vocab: { '[UNK]': 0, hello: 1 }, unk_token: '[UNK]' } });
function transformer(): Files {
  return new Map([
    ['config.json', json({ model_type: 'llama', architectures: ['LlamaForCausalLM'], hidden_size: 1 })],
    ['tokenizer_config.json', json({ tokenizer_class: 'PreTrainedTokenizerFast', unk_token: '[UNK]' })],
    ['tokenizer.json', tokenizer()],
    ['model-00001-of-00002.safetensors', tensor('a')],
    ['model-00002-of-00002.safetensors', tensor('b')],
    ['model.safetensors.index.json', json({ weight_map: {
      a: 'model-00001-of-00002.safetensors', b: 'model-00002-of-00002.safetensors' } })],
  ]);
}
function diffusers(): Files {
  const files: Files = new Map([
    ['model_index.json', json({ _class_name: 'StableDiffusionPipeline',
      unet: ['diffusers', 'UNet2DConditionModel'], vae: ['diffusers', 'AutoencoderKL'],
      text_encoder: ['transformers', 'CLIPTextModel'], tokenizer: ['transformers', 'CLIPTokenizerFast'],
      scheduler: ['diffusers', 'DDIMScheduler'], safety_checker: [null, null], feature_extractor: [null, null] })],
    ['scheduler/scheduler_config.json', json({ _class_name: 'DDIMScheduler', num_train_timesteps: 1000 })],
    ['tokenizer/tokenizer_config.json', json({ tokenizer_class: 'CLIPTokenizerFast' })],
    ['tokenizer/tokenizer.json', tokenizer()],
  ]);
  for (const component of ['unet', 'vae', 'text_encoder']) {
    files.set(`${component}/config.json`, json({ _class_name: 'Model', hidden_size: 1 }));
    files.set(`${component}/model.safetensors`, tensor('weight'));
  }
  return files;
}
function processorPackage(): Files {
  const files = transformer();
  files.set('config.json', json({ model_type: 'whisper', processor_class: 'WhisperProcessor', hidden_size: 1 }));
  files.set('preprocessor_config.json', json({ feature_extractor_type: 'WhisperFeatureExtractor', feature_size: 80 }));
  return files;
}
function sdxl(): Files {
  const files = diffusers(); const index = parsed(bytes(files, 'model_index.json'));
  index['_class_name'] = 'StableDiffusionXLPipeline';
  index['text_encoder_2'] = ['transformers', 'CLIPTextModelWithProjection'];
  index['tokenizer_2'] = ['transformers', 'CLIPTokenizerFast'];
  files.set('model_index.json', json(index));
  files.set('text_encoder_2/config.json', json({ _class_name: 'CLIPTextModelWithProjection', hidden_size: 1 }));
  files.set('text_encoder_2/model.safetensors', tensor('weight'));
  files.set('tokenizer_2/tokenizer_config.json', json({ tokenizer_class: 'CLIPTokenizerFast' }));
  files.set('tokenizer_2/tokenizer.json', tokenizer());
  return files;
}

// A tiny ModelProto whose graph output is an external F32 initializer.
// Field numbers follow https://github.com/onnx/onnx/blob/main/onnx/onnx.proto3.
// This assembles fixture bytes only; no ONNX parser/admission is added to S3.
function externalOnnx(): Files {
  function varint(value: number): Buffer {
    const bytes = [];
    do { bytes.push((value & 127) | (value > 127 ? 128 : 0)); value = Math.floor(value / 128); } while (value);
    return Buffer.from(bytes);
  }
  const integer = (field: number, value: number) => Buffer.concat([varint(field * 8), varint(value)]);
  const message = (field: number, value: Buffer | string) => {
    const bytes = typeof value === 'string' ? Buffer.from(value) : value;
    return Buffer.concat([varint(field * 8 + 2), varint(bytes.length), bytes]);
  };
  const entry = (key: string, value: string) => message(13, Buffer.concat([message(1, key), message(2, value)]));
  const weight = Buffer.concat([integer(1, 1), integer(2, 1), message(8, 'weight'),
    entry('location', 'weights.data'), entry('offset', '0'), entry('length', '4'), integer(14, 1)]);
  const type = message(1, Buffer.concat([integer(1, 1), message(2, message(1, integer(1, 1)))]));
  const output = Buffer.concat([message(1, 'weight'), message(2, type)]);
  const graph = Buffer.concat([message(2, 'owned-fixture'), message(5, weight), message(12, output)]);
  const model = Buffer.concat([integer(1, 8), message(2, 'owned-fixture'), message(7, graph), message(8, integer(2, 13))]);
  const data = Buffer.alloc(4); data.writeFloatLE(1);
  return new Map([['model.onnx', model], ['weights.data', data]]);
}

const cleanup: Array<() => Promise<void>> = [];
afterEach(async () => {
  const failures: unknown[] = [];
  while (cleanup.length) {
    try { await cleanup.pop()?.(); } catch (error) { failures.push(error); }
  }
  delete window.electronAPI;
  if (failures.length) throw new ValidationError(failures.map(String).join('; '), 'fixture-cleanup');
});

async function fixture(files: Files, stall = false) {
  const root = mkdtempSync(resolve(tmpdir(), 'pumas-s3-public-'));
  const traffic: Array<{ method: string; path: string; version: string | null; authorization?: string }> = [];
  let started!: () => void;
  const getStarted = new Promise<void>(done => { started = done; });
  let drained!: () => void;
  const getDrained = new Promise<void>(done => { drained = done; });
  const source = createServer({ pfx: readFileSync(resolve(tls, 'localhost.p12')), passphrase: 'fixture' }, (req, res) => {
    const url = new URL(req.url ?? '/', 'https://localhost');
    if (url.searchParams.has('list-type')) {
      const contents = [...files].sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0).map(([path, bytes]) => `<Contents><Key>models/${path}</Key><ETag>"fixture"</ETag><Size>${bytes.length}</Size></Contents>`).join('');
      const xml = `<ListBucketResult><Name>fixture-bucket</Name><Prefix>models/</Prefix><MaxKeys>16</MaxKeys><KeyCount>${files.size}</KeyCount><IsTruncated>false</IsTruncated>${contents}</ListBucketResult>`;
      res.writeHead(200, { 'Content-Type': 'application/xml', 'Content-Length': Buffer.byteLength(xml) }); res.end(xml); return;
    }
    const path = decodeURIComponent(url.pathname).replace('/fixture-bucket/models/', '');
    const bytes = files.get(path);
    traffic.push({ method: req.method ?? '', path, version: url.searchParams.get('versionId'), authorization: req.headers.authorization });
    if (!bytes) { res.writeHead(404); res.end(); return; }
    const head = req.method === 'HEAD';
    const range = req.headers.range?.match(/^bytes=(\d+)-(\d+)$/);
    const from = range ? Number(range[1]) : 0;
    const to = range ? Number(range[2]) : bytes.length - 1;
    res.writeHead(head ? 200 : 206, { 'Content-Length': head ? bytes.length : to - from + 1,
      'Last-Modified': 'Wed, 01 Jan 2025 00:00:00 GMT', ETag: '"fixture"', 'x-amz-version-id': 'fixture-v1',
      ...(head ? {} : { 'Content-Range': `bytes ${from}-${to}/${bytes.length}` }) });
    if (head) res.end();
    else if (stall) { res.flushHeaders(); res.write(bytes.subarray(from, from + 1)); started(); res.on('close', drained); }
    else res.end(bytes.subarray(from, to + 1));
  });
  source.listen(0, '127.0.0.1'); await once(source, 'listening');
  cleanup.push(async () => { source.closeAllConnections(); await new Promise<void>(done => source.close(() => done())); rmSync(root, { recursive: true, force: true }); });
  const address = source.address();
  if (!address || typeof address === 'string') throw new ValidationError('Missing fixture source address');
  const endpoint = `https://localhost:${address.port}`;
  const child: ChildProcess = spawn(binary, ['--launcher-root', root, '--port', '0'], {
    env: { ...process.env, XDG_CONFIG_HOME: resolve(root, 'config'), SSL_CERT_FILE: resolve(tls, 'localhost.pem') },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let stderr = ''; child.stderr?.on('data', chunk => { stderr += String(chunk); });
  cleanup.push(async () => {
    if (child.exitCode === null && child.signalCode === null) {
      const closed = once(child, 'exit'); child.kill('SIGTERM');
      try { await within(closed, 5000); }
      catch {
        child.kill('SIGKILL');
        await within(closed, 3000);
        throw new ValidationError('Fixture RPC exceeded shutdown grace and required forced termination.', 'fixture-cleanup');
      }
    }
  });
  const port = await new Promise<number>((done, fail) => {
    let output = '';
    const timer = setTimeout(() => fail(new ValidationError(`RPC readiness deadline: ${stderr}`)), 15000);
    child.on('error', error => { clearTimeout(timer); fail(error); });
    child.on('exit', code => { clearTimeout(timer); fail(new ValidationError(`RPC exited ${code}: ${stderr}`)); });
    child.stdout?.on('data', chunk => {
      output += String(chunk); const match = output.match(/RPC_PORT=(\d+)/);
      if (match) { clearTimeout(timer); done(Number(match[1])); }
    });
  });
  const calls: Array<{ method: string; params: Record<string, unknown> }> = [];
  async function rpc(method: string, params: unknown) {
    const response = await fetch(`http://127.0.0.1:${port}/rpc`, { method: 'POST',
      signal: AbortSignal.timeout(10000),
      headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }) });
    const raw: unknown = await response.json(); const value = record(raw);
    if (value['error']) throw new ValidationError(`RPC ${method} refused: ${JSON.stringify(value['error'])}`, 'rpc-fixture');
    return value['result'];
  }
  async function persisted() {
    const decoded = decodeS3PersistedImportsWire(await rpc('inspect_persisted_s3_imports', {}));
    if (decoded.status !== 'valid' || decoded.value.status !== 'complete') {
      throw new ValidationError('Incomplete fixture acquisition observation.', 'fixture');
    }
    return decoded.value.imports;
  }
  runInNewContext(preload, { module: { exports: {} }, exports: {}, console,
    require: (name: string) => {
      if (name !== 'electron') throw new ValidationError(`Unexpected preload dependency ${name}`);
      return { contextBridge: { exposeInMainWorld: (name: string, bridge: typeof window.electronAPI) => {
        expect(name).toBe('electronAPI'); window.electronAPI = bridge;
      } }, ipcRenderer: { on: () => {}, removeListener: () => {}, sendSync: () => ({
        status: 'ready', selectionAction: 'select-library', libraryScopeId: null }),
      invoke: async (channel: string, method: string, params: unknown) => {
        expect(channel).toBe('api:call'); const request = validateApiCallPayload(method, params);
        calls.push(request);
        return receiveS3ImportRpc(method, () => rpc(request.method, request.params));
      } } };
    },
  });
  return { root, endpoint, calls, traffic, rpc, persisted, getStarted, getDrained };
}

function change(label: string, value: string) { fireEvent.change(screen.getByLabelText(label), { target: { value } }); }
async function select(files: Files, primary: string, endpoint: string, discover = false, authenticated = false) {
  await waitFor(() => expect(screen.getByRole('button', { name: 'Import pinned object' })).toBeEnabled());
  for (const [label, value] of [['HTTPS endpoint origin', endpoint], ['Region', 'fixture-region'], ['Bucket', 'fixture-bucket'],
    ['Model family', 'fixture'], ['Model name', 'public-model']] as const) change(label, value);
  if (discover) {
    fireEvent.click(screen.getByRole('button', { name: 'Discover prefix' }));
    await screen.findByText(new RegExp(`Complete discovery: ${files.size} objects`), {}, { timeout: 10000 });
    fireEvent.click(screen.getByRole('button', { name: `Use models/${primary} as primary` }));
  } else { change('Exact object key', `models/${primary}`); change('Immutable VersionId', 'fixture-v1'); }
  change('Primary weight logical path', primary); change('Expected SHA-256', digest(bytes(files, primary)));
  let row = 0;
  for (const [path, bytes] of files) {
    if (path === primary) continue;
    if (discover) fireEvent.click(screen.getByRole('button', { name: `Add models/${path} as package file` }));
    else fireEvent.click(screen.getByRole('button', { name: 'Add package file' }));
    row++;
    if (!discover) { change(`Package file ${row} exact object key`, `models/${path}`); change(`Package file ${row} immutable VersionId`, 'fixture-v1'); }
    change(`Package file ${row} logical output path`, path); change(`Package file ${row} expected SHA-256`, digest(bytes));
  }
  if (authenticated) {
    fireEvent.click(screen.getByLabelText('Use one-use credentials'));
    change('Access key ID', 'public-fixture-key'); change('Secret access key', 'public-fixture-secret');
  }
  fireEvent.click(screen.getByRole('button', { name: 'Import pinned object' }));
}

describe('public non-GGUF registration through actual desktop layers', () => {
  for (const [name, make, primary, discover, authenticated] of [
    ['legacy single GGUF', () => new Map([['weights.gguf', gguf()]]), 'weights.gguf', false, false],
    ['legacy GGUF with inert index auxiliary', () => new Map([['weights.gguf', gguf()], ['model_index.json', json({ _class_name: 'InertAuxiliary' })]]), 'weights.gguf', false, false],
    ['single safetensors', () => new Map([['model.safetensors', tensor('weight')]]), 'model.safetensors', false, false],
    ['complete sharded Transformers', transformer, 'model-00001-of-00002.safetensors', true, true],
    ['complete Diffusers components', diffusers, 'unet/model.safetensors', true, false],
    ['complete SDXL components', sdxl, 'unet/model.safetensors', true, false],
    ['required processor package', processorPackage, 'model-00001-of-00002.safetensors', false, false],
  ] as const) it(`registers ${name} with exact paths and byte evidence`, async () => {
    const files = make(); const env = await fixture(files); const imported = vi.fn();
    render(<S3ModelImportDialog onClose={vi.fn()} onImported={imported} />);
    await select(files, primary, env.endpoint, discover, authenticated);
    const registered = await screen.findByText(/^Model registered:/, {}, { timeout: 15000 });
    const id = registered.textContent?.replace('Model registered: ', '');
    if (!id) throw new ValidationError('Missing registered fixture identity.', 'fixture');
    expect(screen.getByText(/Registration does not establish backend compatibility/)).toBeInTheDocument();
    await waitFor(() => expect(imported).toHaveBeenCalledTimes(1));
    for (const [path, bytes] of files) expect(readFileSync(resolve(env.root, 'shared-resources/models', id, path))).toEqual(bytes);
    const metadata = parsed(readFileSync(resolve(env.root, 'shared-resources/models', id, 'metadata.json')));
    expect(metadata['import_state']).toBe('ready');
    expect(env.traffic.filter(call => call.method === 'GET')).toHaveLength(files.size);
    expect(env.traffic.filter(call => call.method === 'GET').every(call => call.version === 'fixture-v1')).toBe(true);
    const starts = env.calls.filter(call => call.method.startsWith('start_') && call.method.endsWith('_import'));
    expect(starts).toHaveLength(1);
    const records = await env.persisted(); expect(records).toHaveLength(1);
    expect(records[0]?.phase).toBe('adopted'); expect(records[0]?.receipt_present).toBe(true);
    expect(records[0]?.model_binding?.model_id).toBe(id);
    expect(records[0]?.model_binding?.publication_state).toBe('confirmed');
    if (authenticated) {
      expect(starts[0]?.method).toBe('start_authenticated_s3_model_bundle_import');
      expect(env.traffic.filter(call => call.method === 'GET').every(call => call.authorization?.includes('public-fixture-key'))).toBe(true);
      expect(screen.getByLabelText('Secret access key')).toHaveValue('');
    }
  }, 30000);

  for (const [name, make, primary] of [
    ['ONNX external tensor package', externalOnnx, 'model.onnx'],
    ['missing shard', () => { const files = transformer(); files.delete('model-00002-of-00002.safetensors'); return files; }, 'model-00001-of-00002.safetensors'],
    ['missing processor', () => { const files = processorPackage(); files.delete('preprocessor_config.json'); return files; }, 'model-00001-of-00002.safetensors'],
    ['malformed safetensors', () => { const files = transformer(); files.set('model-00001-of-00002.safetensors', Buffer.from('malformed')); return files; }, 'model-00001-of-00002.safetensors'],
    ['missing Diffusers component', () => { const files = diffusers(); files.delete('vae/model.safetensors'); return files; }, 'unet/model.safetensors'],
    ['unsupported pipeline class', () => { const files = diffusers(); const index = parsed(bytes(files, 'model_index.json')); index['_class_name'] = 'FluxPipeline'; files.set('model_index.json', json(index)); return files; }, 'unet/model.safetensors'],
    ['custom code', () => { const files = transformer(); files.set('config.json', json({ model_type: 'llama', auto_map: { AutoModel: 'custom.Model' } })); return files; }, 'model-00001-of-00002.safetensors'],
  ] as const) it(`acquires selected bytes and refuses ${name} before registration`, async () => {
    const files = make(); const env = await fixture(files); const imported = vi.fn();
    render(<S3ModelImportDialog onClose={vi.fn()} onImported={imported} />);
    await select(files, primary, env.endpoint);
    await screen.findByText(/other retained work requires reconciliation/, {}, { timeout: 15000 });
    expect(screen.queryByText(/^Model registered:/)).not.toBeInTheDocument(); expect(imported).not.toHaveBeenCalled();
    expect(env.traffic.filter(call => call.method === 'GET')).toHaveLength(files.size);
    const outcome = record(await env.rpc('get_s3_model_import', {}));
    expect(outcome['status']).toBe('finished'); const result = record(outcome['result']);
    expect(result['status']).toBe('failed'); expect(result['retained_work']).toBe(true); expect(result['published_model_id']).toBeNull();
    const models = record(await env.rpc('get_models', {})); expect(Object.keys(record(models['models']))).toHaveLength(0);
    const records = await env.persisted(); expect(records).toHaveLength(1);
    expect(records[0]?.phase).toBe('using'); expect(records[0]?.receipt_present).toBe(true);
    expect(records[0]?.model_binding).toBeNull();
    expect(screen.getByRole('button', { name: 'Import pinned object' })).toBeDisabled();
  }, 30000);

  it('cancels a non-GGUF transfer through the public UI and drains it before publication', async () => {
    const files = new Map([['model.safetensors', tensor('weight')]]); const env = await fixture(files, true);
    const imported = vi.fn(); render(<S3ModelImportDialog onClose={vi.fn()} onImported={imported} />);
    await select(files, 'model.safetensors', env.endpoint); await within(env.getStarted, 10000);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Cancel import' })).toBeEnabled());
    fireEvent.click(screen.getByRole('button', { name: 'Cancel import' }));
    await screen.findByText('Import cancelled.', {}, { timeout: 10000 }); await within(env.getDrained, 10000);
    expect(imported).not.toHaveBeenCalled(); expect(screen.queryByText(/^Model registered:/)).not.toBeInTheDocument();
    expect(Object.keys(record(record(await env.rpc('get_models', {}))['models']))).toHaveLength(0);
    expect(env.calls.filter(call => call.method === 'cancel_s3_model_import')).toHaveLength(1);
    expect((await env.persisted()).every(row => !row.receipt_present)).toBe(true);
  }, 30000);

  it('refuses reserved single-file destinations before any source or acquisition effect', async () => {
    const files = new Map([['metadata.json', tensor('weight')]]); const env = await fixture(files);
    render(<S3ModelImportDialog onClose={vi.fn()} />);
    await select(files, 'metadata.json', env.endpoint);
    await screen.findByText(/The operation result is unavailable/, {}, { timeout: 10000 });
    expect(env.traffic).toHaveLength(0);
    expect(await env.persisted()).toHaveLength(0);
    expect(Object.keys(record(record(await env.rpc('get_models', {}))['models']))).toHaveLength(0);
    const starts = env.calls.filter(call => call.method === 'start_s3_model_import');
    expect(starts).toHaveLength(1);
    await expect(env.rpc('start_s3_model_import', starts[0]?.params)).rejects.toThrow(/-32602/);
    expect(env.traffic).toHaveLength(0);
  }, 30000);
});
