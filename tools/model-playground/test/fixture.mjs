// Test-only loopback protocol simulator. Never used by the playground launcher.
import http from 'node:http';
export const PNG = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a5S0AAAAASUVORK5CYII=';
export const FENCE = { instance: 'fixture-owner', service: 'fixture-service' };
export function capabilities(model = 'fixture/model', profile = 'fixture-profile') {
  return { supported_contract_versions: [1], model, profile, max_request_bytes: 33554432,
    max_response_bytes: 33554432, max_stream_event_bytes: 262144,
    capabilities: [
      ['chat_generation', 'chat_generation', ['messages_text'], 'text'],
      ['text_generation', 'text_generation', ['text'], 'text'],
      ['text_embedding', 'text_embedding', ['text', 'text_batch'], 'embeddings_float32'],
      ['image_generation', 'text_to_image', ['text'], 'png_base64'],
      ['image_to_text', 'image_to_text', ['png_base64', 'jpeg_base64', 'messages_image'], 'text'],
      ['audio_transcription', 'speech_to_text', ['pcm_s16le', 'pcm_f32le'], 'text'],
      ['audio_classification', 'audio_classification', ['pcm_s16le'], 'labels'],
    ].map(([capability, semantic_task, input_formats, output]) => ({ capability, semantic_task, input_formats,
      output_formats: [output], streaming: ['chat_generation', 'text_generation'].includes(capability),
      availability: capability === 'audio_classification' ? { state: 'unavailable', reason: 'unsupported_adapter' } : { state: 'available' }, option_bounds: [] })) };
}
export async function startFixture() {
  const state = { mode: 'success', operations: [], requests: [], disconnects: 0, pending: new Set() };
  const server = http.createServer(async (req, res) => {
    const chunks = []; for await (const chunk of req) chunks.push(chunk);
    const body = chunks.length ? JSON.parse(Buffer.concat(chunks)) : null;
    state.requests.push({ path: req.url, headers: req.headers, body });
    const json = (value, status = 200) => { res.writeHead(status, { 'content-type': 'application/json' }); res.end(JSON.stringify(value)); };
    if (req.url === '/.well-known/pumas') return json({ advertisement_schema_version: 1, endpoint: state.endpoint,
      instance: { generation: FENCE.instance }, service_generation: FENCE.service,
      build_info: { component: 'SIMULATED contract fixture — not inference', package_version: 'test-only', schemas: [{ name: 'pumas.http-admission-fence', version: 1 }] } });
    if (req.headers['pumas-instance-generation'] !== FENCE.instance || req.headers['pumas-service-generation'] !== FENCE.service || state.mode === 'stale') return json({ error: 'stale fixture fence' }, 412);
    if (req.url === '/v1/models') return json({ object: 'list', data: state.mode === 'empty' ? [] : [{ id: 'fixture/model', object: 'model' }] });
    if (req.url === '/rpc') return json({ jsonrpc: '2.0', id: body.id, result: { success: true, plugins: [{ id: 'torch', displayName: 'Simulated Torch manifest', installationType: 'python-venv' }] } });
    if (req.url.startsWith('/v1/capabilities?')) {
      const value = capabilities();
      if (state.mode === 'unavailable') value.capabilities.forEach(c => { c.availability = { state: 'unavailable', reason: 'runtime_unavailable' }; });
      if (state.mode === 'malformed-capabilities') value.capabilities[0].availability = {};
      return json(value);
    }
    if (req.url === '/v1/model-operations') {
      state.operations.push(body);
      if (state.mode === 'hang') { state.pending.add(res); res.once('close', () => { state.disconnects++; state.pending.delete(res); }); return; }
      if (state.mode === 'disconnect') { req.socket.destroy(); return; }
      if (state.mode === 'redirect') { res.writeHead(307, { location: '/v1/model-operations' }); res.end(); return; }
      if (state.mode === 'oversize') { res.writeHead(200, { 'content-type': 'application/json' }); res.end('x'.repeat(4096)); return; }
      if (state.mode === 'not-json') { res.writeHead(200, { 'content-type': 'text/plain' }); res.end('Bearer fixture-secret'); return; }
      if (state.mode === 'provider-error') return json({ contract_version: 1, request_id: body.request_id, error: { code: 'provider_failure', outcome: 'unknown' } }, 502);
      if (state.mode === 'not-admitted') return json({ contract_version: 1, request_id: body.request_id, error: { code: 'capability_unavailable', outcome: 'not_admitted' } }, 503);
      const result = body.output === 'png_base64' ? { kind: 'image', png_base64: state.mode === 'invalid-image' ? 'iVBORw0KGgo=' : PNG, seed: 42 } : body.output === 'embeddings_float32' ?
        { kind: 'embeddings', vectors: (body.input.texts ?? [body.input.text]).map(() => [0.25, -0.5]) } :
        { kind: 'text', text: 'SIMULATED adapter response. No model inference occurred.', finish_reason: 'stop' };
      return json({ contract_version: 1, request_id: state.mode === 'wrong-id' ? 'wrong-id' : body.request_id, result });
    }
    json({ error: 'fixture route not found' }, 404);
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  state.endpoint = `http://127.0.0.1:${server.address().port}`;
  return { state, server, async close() { const closed = new Promise(resolve => server.close(resolve)); server.closeAllConnections(); await closed; } };
}
