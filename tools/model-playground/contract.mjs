// Consumer subset of the public v1 DTOs; Pumas remains the admission authority.
export const LIMITS = Object.freeze({ requestBytes: 16 * 1024 * 1024,
  responseBytes: 32 * 1024 * 1024, fileBytes: 8 * 1024 * 1024,
  maxTokens: 512, imagePixels: 1024 * 1024 });
export const MODALITIES = Object.freeze([
  ['chat_generation', 'Chat'], ['text_generation', 'Text'],
  ['text_embedding', 'Embeddings'], ['audio_transcription', 'Transcription'],
  ['audio_generation', 'Audio generation'], ['image_generation', 'Image generation'],
  ['image_to_text', 'Image to text'], ['image_editing', 'Image editing'],
  ['generation_3d', '3D generation'], ['audio_classification', 'Audio classification'],
]);
export const ADAPTERS = new Set(['chat_generation', 'text_generation', 'text_embedding',
  'audio_transcription', 'image_generation', 'image_to_text']);
const ERROR_CODES = new Set(['invalid_request', 'unsupported_contract', 'model_not_found',
  'ambiguous_model', 'ambiguous_operation', 'unsupported_modality', 'capability_unavailable',
  'provider_failure', 'invalid_provider_result', 'request_limit', 'response_limit', 'transport_lost']);
export class PlaygroundError extends Error {
  constructor(code, message, outcome = 'not_admitted') {
    super(message); this.code = code; this.outcome = outcome;
  }
}
export function assert(condition, message, code = 'invalid_request') {
  if (!condition) throw new PlaygroundError(code, message);
}
export const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
function closed(value, required, optional = []) {
  assert(object(value) && required.every(key => Object.hasOwn(value, key)) &&
    Object.keys(value).every(key => [...required, ...optional].includes(key)), 'Invalid or unexpected fields.');
}
function text(value, max = 65536, min = 0) {
  assert(typeof value === 'string' && value.length >= min && value.length <= max, 'Text length is outside the supported bound.');
}
function number(value, min, max, integer = true) {
  assert(typeof value === 'number' && Number.isFinite(value) && (!integer || Number.isInteger(value)) &&
    value >= min && value <= max, `Expected ${integer ? 'integer' : 'number'} in [${min}, ${max}].`);
}
export function identity(value, name = 'model') {
  text(value, 256, 1);
  assert(value.trim() === value && !/[\x00-\x20\x7f]/.test(name === 'profile' ? value : value.replaceAll(' ', '')) &&
    new TextEncoder().encode(value).length <= 256, `Invalid ${name} identity.`);
  return value;
}
function bytes(base64) {
  assert(typeof base64 === 'string' && base64.length <= Math.ceil(LIMITS.fileBytes / 3) * 4 &&
    base64.length % 4 === 0 && !/[^A-Za-z0-9+/=]/.test(base64), 'Expected bounded standard base64.');
  let decoded;
  try { decoded = atob(base64); } catch { throw new PlaygroundError('invalid_request', 'Invalid base64 padding or encoding.'); }
  assert(decoded.length <= LIMITS.fileBytes, 'Decoded file exceeds the byte bound.', 'request_limit');
  return Uint8Array.from(decoded, c => c.charCodeAt(0));
}
function image(input) {
  closed(input, ['kind', 'encoding', 'data_base64']);
  assert(input.kind === 'image' && ['png', 'jpeg'].includes(input.encoding), 'Expected PNG or JPEG image.');
  const data = bytes(input.data_base64);
  assert(data.length > 0 && (input.encoding === 'png'
    ? [137, 80, 78, 71, 13, 10, 26, 10].every((b, i) => data[i] === b)
    : data[0] === 255 && data[1] === 216 && data[2] === 255), 'Image signature does not match encoding.');
}
function textOptions(options) {
  closed(options, ['kind', 'max_tokens'], ['temperature', 'top_p']);
  assert(options.kind === 'text_generation', 'Expected text_generation options.');
  number(options.max_tokens, 1, LIMITS.maxTokens);
  if (options.temperature !== undefined) number(options.temperature, 0, 2, false);
  if (options.top_p !== undefined) number(options.top_p, 0, 1, false);
}
export function validateRequest(value) {
  closed(value, ['contract_version', 'request_id', 'model', 'profile', 'capability', 'input', 'output', 'options', 'stream']);
  assert(value.contract_version === 1 && value.stream === false, 'This harness supports finite contract v1 operations only.');
  assert(typeof value.request_id === 'string' && /^[A-Za-z0-9_.:-]{1,128}$/.test(value.request_id), 'Invalid request ID.');
  identity(value.model); identity(value.profile, 'profile');
  assert(ADAPTERS.has(value.capability), 'No reviewed public adapter for this capability.', 'unsupported_adapter');
  const { input, options, capability } = value;
  if (capability === 'chat_generation') {
    closed(input, ['kind', 'messages']);
    assert(input.kind === 'messages' && Array.isArray(input.messages) && input.messages.length > 0 && input.messages.length <= 32, 'Expected 1–32 text messages.');
    for (const message of input.messages) {
      closed(message, ['role', 'content']);
      assert(['system', 'user', 'assistant'].includes(message.role), 'Unsupported message role.');
      text(message.content);
    }
    textOptions(options);
  } else if (capability === 'text_generation' || capability === 'image_generation') {
    closed(input, ['kind', 'text']); assert(input.kind === 'text', 'Expected text input.');
    text(input.text, capability === 'image_generation' ? 4000 : 65536, 1);
    if (capability === 'text_generation') textOptions(options);
    else {
      closed(options, ['kind', 'width', 'height'], ['seed']);
      assert(options.kind === 'image_generation', 'Expected image_generation options.');
      number(options.width, 1, 2048); number(options.height, 1, 2048);
      assert(options.width * options.height <= LIMITS.imagePixels, 'Playground image limit is 1,048,576 pixels.');
      if (options.seed !== undefined) number(options.seed, 0, 4294967295);
    }
  } else if (capability === 'text_embedding') {
    assert(object(input), 'Expected text or text_batch.');
    const texts = input.kind === 'text' ? [input.text] : input.texts;
    closed(input, ['kind', input.kind === 'text' ? 'text' : 'texts']);
    assert(['text', 'text_batch'].includes(input.kind) && Array.isArray(texts) && texts.length >= 1 && texts.length <= 128, 'Expected 1–128 texts.');
    texts.forEach(item => text(item));
    assert(texts.reduce((n, item) => n + item.length, 0) <= 65536, 'Embedding input exceeds 65,536 characters.');
    closed(options, ['kind'], ['dimensions']);
    assert(options.kind === 'embeddings', 'Expected embeddings options.');
    if (options.dimensions !== undefined) number(options.dimensions, 1, 8192);
  } else if (capability === 'image_to_text') {
    closed(input, ['kind', 'messages']);
    assert(input.kind === 'image_messages' && Array.isArray(input.messages) && input.messages.length === 1, 'Playground accepts one image message.');
    const message = input.messages[0];
    closed(message, ['role', 'content']);
    assert(message.role === 'user' && Array.isArray(message.content) && message.content.length === 2, 'Expected one user message with text and image.');
    closed(message.content[0], ['kind', 'text']);
    assert(message.content[0].kind === 'text', 'Expected a text prompt.'); text(message.content[0].text);
    image(message.content[1]); textOptions(options);
  } else if (capability === 'audio_transcription') {
    closed(input, ['kind', 'encoding', 'sample_rate_hz', 'channels', 'sample_count', 'data_base64']);
    assert(input.kind === 'audio' && ['pcm_s16le', 'pcm_f32le'].includes(input.encoding), 'Expected raw little-endian PCM. WAV/MP3 containers must be converted explicitly.');
    number(input.sample_rate_hz, 8000, 192000); number(input.channels, 1, 2);
    number(input.sample_count, 1, input.sample_rate_hz * 30);
    const data = bytes(input.data_base64);
    const sampleBytes = input.encoding === 'pcm_s16le' ? 2 : 4;
    assert(data.length === input.sample_count * input.channels * sampleBytes, 'PCM byte length and declared frames do not match.');
    if (sampleBytes === 4) {
      const view = new DataView(data.buffer);
      for (let i = 0; i < data.length; i += 4) number(view.getFloat32(i, true), -1, 1, false);
    }
    closed(options, ['kind', 'max_output_tokens'], ['language']);
    assert(options.kind === 'audio', 'Expected audio options.'); number(options.max_output_tokens, 1, 512);
    if (options.language !== undefined) assert(['en', 'de', 'fr', 'it', 'es', 'pt', 'el', 'nl', 'pl', 'vi', 'zh', 'ar', 'ja', 'ko'].includes(options.language), 'Unsupported language.');
  }
  const output = capability === 'text_embedding' ? 'embeddings_float32' : capability === 'image_generation' ? 'png_base64' : 'text';
  assert(value.output === output, 'Output does not match the selected capability.');
  assert(new TextEncoder().encode(JSON.stringify(value)).length <= LIMITS.requestBytes, 'Request exceeds playground byte limit.', 'request_limit');
  return value;
}
export function decodeCapabilities(value, model, profile) {
  try { return decodeCapabilityValue(value, model, profile); }
  catch (error) {
    if (error.code === 'unsupported_contract') throw error;
    throw new PlaygroundError('invalid_response', `Invalid capabilities: ${error.message}`);
  }
}
function decodeCapabilityValue(value, model, profile) {
  assert(object(value) && Array.isArray(value.supported_contract_versions) && value.supported_contract_versions.includes(1), 'Contract v1 capabilities unavailable.', 'unsupported_contract');
  assert(value.model === model && (!profile || value.profile === profile), 'Capability identity mismatch.', 'invalid_response');
  identity(value.profile, 'profile');
  for (const key of ['max_request_bytes', 'max_response_bytes', 'max_stream_event_bytes']) number(value[key], 1, Number.MAX_SAFE_INTEGER);
  assert(Array.isArray(value.capabilities) && value.capabilities.length <= 64, 'Invalid capability list.', 'invalid_response');
  const names = new Set();
  for (const item of value.capabilities) {
    assert(object(item) && typeof item.capability === 'string' && !names.has(item.capability) &&
      typeof item.semantic_task === 'string' && typeof item.streaming === 'boolean' &&
      [item.input_formats, item.output_formats].every(list => Array.isArray(list) && list.every(x => typeof x === 'string')) &&
      object(item.availability) && ['available', 'unavailable'].includes(item.availability.state) &&
      (item.availability.state !== 'unavailable' || typeof item.availability.reason === 'string') &&
      Array.isArray(item.option_bounds), 'Malformed capability descriptor.', 'invalid_response');
    names.add(item.capability);
    for (const bound of item.option_bounds) assert(object(bound) && typeof bound.option === 'string' &&
      Number.isFinite(bound.minimum) && Number.isFinite(bound.maximum) && bound.minimum <= bound.maximum, 'Invalid option bounds.', 'invalid_response');
  }
  return value;
}
export function capabilityRows(discovery) {
  const known = new Map(MODALITIES);
  for (const item of discovery?.capabilities ?? []) if (!known.has(item.capability)) known.set(item.capability, item.capability);
  return [...known].map(([capability, label]) => {
    const descriptor = discovery?.capabilities.find(item => item.capability === capability);
    const supported = ADAPTERS.has(capability) && descriptor?.availability.state === 'available';
    return { capability, label, support: supported ? 'supported' : discovery ? 'unavailable' : 'untested',
      execution: 'untested', reason: !discovery ? 'Inspect a served model.' : !descriptor ? 'Not advertised by this runtime.' :
        descriptor.availability.state === 'unavailable' ? descriptor.availability.reason :
          !ADAPTERS.has(capability) ? 'No reviewed playground adapter.' : 'Runtime advertises availability; inference is untested.', descriptor };
  });
}
export function admit(request, capabilities) {
  validateRequest(request);
  assert(request.model === capabilities.model && request.profile === capabilities.profile, 'Selected serving identity changed.');
  const descriptor = capabilities.capabilities.find(item => item.capability === request.capability);
  assert(descriptor?.availability.state === 'available', descriptor?.availability.reason ?? 'Capability not advertised.', 'capability_unavailable');
  const format = request.input.kind === 'messages' ? 'messages_text' : request.input.kind === 'image_messages' ? 'messages_image' :
    request.input.kind === 'audio' ? request.input.encoding : request.input.kind;
  assert(descriptor.input_formats.includes(format) && descriptor.output_formats.includes(request.output), 'Advertised input/output format is incompatible.', 'unsupported_contract');
  for (const bound of descriptor.option_bounds) {
    const value = request.options[bound.option];
    if (value !== undefined) number(value, bound.minimum, bound.maximum, false);
  }
  assert(new TextEncoder().encode(JSON.stringify(request)).length <= capabilities.max_request_bytes, 'Runtime request byte limit exceeded.', 'request_limit');
}
export function decodeResult(value, request) {
  assert(object(value) && value.contract_version === 1 && value.request_id === request.request_id, 'Response correlation/version mismatch.', 'invalid_response');
  if (value.error) {
    assert(object(value.error) && ERROR_CODES.has(value.error.code) && ['not_admitted', 'unknown'].includes(value.error.outcome), 'Malformed or unsupported operation error.', 'invalid_response');
    return { state: 'error', error: { code: value.error.code, outcome: value.error.outcome } };
  }
  const result = value.result;
  assert(object(result), 'Missing typed result.', 'invalid_response');
  if (request.output === 'text') {
    assert(result.kind === 'text' && typeof result.text === 'string' && ['stop', 'length', 'content_filter'].includes(result.finish_reason), 'Invalid text result.', 'invalid_response');
  } else if (request.output === 'embeddings_float32') {
    const count = request.input.kind === 'text_batch' ? request.input.texts.length : 1;
    assert(result.kind === 'embeddings' && Array.isArray(result.vectors) && result.vectors.length === count &&
      result.vectors.every(v => Array.isArray(v) && v.length > 0 && v.length <= 8192 &&
        v.length === result.vectors[0].length && (!request.options.dimensions || v.length === request.options.dimensions) &&
        v.every(n => typeof n === 'number' && Number.isFinite(n) && Math.abs(n) <= 3.4028234663852886e38)), 'Incoherent embedding result.', 'invalid_response');
  } else {
    assert(result.kind === 'image', 'Invalid image result.', 'invalid_response');
    image({ kind: 'image', encoding: 'png', data_base64: result.png_base64 }); number(result.seed, 0, 4294967295);
  }
  // Project only validated public fields; additive provider fields remain in redacted receipts.
  const projected = result.kind === 'text' ? { kind: result.kind, text: result.text, finish_reason: result.finish_reason } :
    result.kind === 'embeddings' ? { kind: result.kind, vectors: result.vectors } :
      { kind: result.kind, png_base64: result.png_base64, seed: result.seed };
  return { state: 'succeeded', result: projected };
}
export function template(capability, model, profile, requestId) {
  const request = { contract_version: 1, request_id: requestId, model, profile, capability,
    input: { kind: 'text', text: 'Describe a quiet forest in one sentence.' }, output: 'text',
    options: { kind: 'text_generation', max_tokens: 128 }, stream: false };
  if (capability === 'chat_generation') request.input = { kind: 'messages', messages: [{ role: 'user', content: request.input.text }] };
  if (capability === 'text_embedding') { request.output = 'embeddings_float32'; request.options = { kind: 'embeddings' }; }
  if (capability === 'image_generation') { request.output = 'png_base64'; request.options = { kind: 'image_generation', width: 512, height: 512, seed: 42 }; }
  if (capability === 'audio_transcription') { request.input = { kind: 'audio', encoding: 'pcm_s16le', sample_rate_hz: 16000, channels: 1, sample_count: 0, data_base64: '' }; request.options = { kind: 'audio', max_output_tokens: 512 }; }
  if (capability === 'image_to_text') request.input = { kind: 'image_messages', messages: [{ role: 'user', content: [{ kind: 'text', text: 'Describe this image.' }, { kind: 'image', encoding: 'png', data_base64: '' }] }] };
  return request;
}
