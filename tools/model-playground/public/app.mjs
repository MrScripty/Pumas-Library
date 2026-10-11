import { LIMITS, capabilityRows, template, validateRequest } from '/contract.mjs';

const $ = id => document.getElementById(id);
const session = document.querySelector('meta[name="playground-session"]').content;
let discovery = null, capabilities = null, selected = null, active = null, receiptData = null, imageUrl = null;
const evidence = new Map();
function key(capability) { return JSON.stringify([discovery?.fence, capabilities?.model, capabilities?.profile, capability]); }
function message(text, error = false) { $('status').textContent = text; $('status').className = error ? 'error' : ''; }
function controls() {
  $('discover').disabled = !!active; $('inspect').disabled = !!active || !$('model').value || !discovery;
  $('model').disabled = !!active || !discovery?.models.length; $('profile').disabled = !!active;
  $('run').disabled = !!active || !selected; $('cancel').disabled = !active;
  $('request').disabled = !!active || !selected; $('file').disabled = !!active;
  renderCapabilities();
}
function renderCapabilities() {
  $('capabilities').replaceChildren();
  for (const row of capabilityRows(capabilities)) {
    const card = document.createElement('div'); card.className = `capability${selected === row.capability ? ' active' : ''}`;
    const button = document.createElement('button'); button.textContent = row.label;
    button.disabled = !!active || row.support !== 'supported'; button.setAttribute('aria-pressed', String(selected === row.capability));
    button.addEventListener('click', () => choose(row.capability));
    const state = document.createElement('div'); state.className = `badge ${row.support}`;
    state.textContent = `${row.support.toUpperCase()} · RUN: ${evidence.get(key(row.capability)) ?? 'UNTESTED'}`;
    const reason = document.createElement('p'); reason.textContent = evidence.get(key(row.capability)) === 'SUCCEEDED' && row.support === 'supported'
      ? 'A terminal result was validated in this session.' : row.reason;
    card.append(button, state, reason); $('capabilities').append(card);
  }
}
function clearSelection() { capabilities = null; selected = null; $('request').value = ''; $('selected').textContent = 'Choose a supported capability'; $('attachment').hidden = true; controls(); }
function choose(capability) {
  if (imageUrl) { URL.revokeObjectURL(imageUrl); imageUrl = null; }
  $('output').textContent = 'No response for this request yet.'; $('result-state').textContent = 'UNTESTED'; $('result-state').className = 'badge';
  selected = capability; $('selected').textContent = capability; $('file').value = '';
  $('request').value = JSON.stringify(template(capability, capabilities.model, capabilities.profile, crypto.randomUUID()), null, 2);
  $('attachment').hidden = !['audio_transcription', 'image_to_text'].includes(capability);
  $('file').accept = capability === 'image_to_text' ? 'image/png,image/jpeg' : '.pcm,.raw';
  $('file-help').textContent = capability === 'image_to_text' ? 'PNG or JPEG, up to 8 MiB. The gateway validates dimensions and content.' :
    'Raw PCM only, up to 30 seconds / 8 MiB. Set encoding, channels and sample rate in JSON before attaching; frames are computed from byte length. No relabeling or conversion.';
  controls();
}
async function api(path, input) {
  const response = await fetch(path, { method: 'POST', headers: { 'content-type': 'application/json', 'x-playground-session': session }, body: JSON.stringify(input) });
  return response.json();
}
function showReceipts(value) {
  receiptData = { format: 'pumas-playground-receipt-v1', captured_at: new Date().toISOString(),
    endpoint: discovery?.endpoint, build: discovery?.build, fence: discovery?.fence,
    evidence: 'Actual HTTP exchanges; inference success is only a validated terminal result.',
    error: value.error, receipts: value.receipts ?? [] };
  $('receipts').textContent = JSON.stringify(receiptData, null, 2); $('export').disabled = false;
}
async function invoke(path, input, consume) {
  if (active) return;
  let observedResponse;
  const id = crypto.randomUUID(); active = { id }; controls();
  message(path === '/api/run' ? 'Request running. A silent generation may still be working.' : 'Reading runtime observations…');
  try {
    const value = await api(path, { id, ...input });
    observedResponse = value;
    showReceipts(value);
    if (value.error) {
      message(`${value.error.code}: ${value.error.message ?? value.error.outcome} (${value.error.outcome})`, true);
      if (path === '/api/discover') { $('runtime').textContent = 'Runtime unavailable or incompatible'; $('plugins').textContent = 'Plugin inventory unavailable; discovery failed.'; }
      if (path === '/api/run') { evidence.set(key(selected), value.error.outcome === 'unknown' ? 'UNKNOWN' : 'ERROR'); await showResult(value); }
    } else { await consume(value); }
  } catch (error) {
    const value = { receipts: observedResponse?.receipts ?? [], error: { code: error.code === 'invalid_image' ? 'invalid_image' : 'playground_transport_lost', outcome: path === '/api/run' ? 'unknown' : 'not_admitted', message:
      error.code === 'invalid_image' ? 'The returned PNG could not be decoded. Inspect the receipt; no automatic retry.' : 'Playground connection lost. No automatic retry.' } };
    showReceipts(value); message(value.error.message, true);
    if (path === '/api/run') { evidence.set(key(selected), 'UNKNOWN'); await showResult(value); }
  } finally { active = null; controls(); }
}
async function showResult(value) {
  if (imageUrl) { URL.revokeObjectURL(imageUrl); imageUrl = null; }
  $('output').replaceChildren();
  $('result-state').textContent = value.state === 'succeeded' ? 'SUCCEEDED' : 'ERROR / UNKNOWN';
  $('result-state').className = `badge ${value.state === 'succeeded' ? 'succeeded' : 'error'}`;
  if (value.error) { $('output').textContent = JSON.stringify(value.error, null, 2); return; }
  if (value.result.kind === 'image') {
    const bytes = Uint8Array.from(atob(value.result.png_base64), c => c.charCodeAt(0));
    imageUrl = URL.createObjectURL(new Blob([bytes], { type: 'image/png' }));
    const image = document.createElement('img'); image.src = imageUrl; image.alt = 'Image returned by the selected Pumas model';
    try { await image.decode(); } catch { const error = new Error('Invalid PNG'); error.code = 'invalid_image'; throw error; }
    const link = document.createElement('a'); link.href = imageUrl; link.download = 'pumas-result.png'; link.textContent = `Download PNG · seed ${value.result.seed}`;
    $('output').append(image, link);
  } else $('output').textContent = value.result.kind === 'text' ? value.result.text : JSON.stringify(value.result, null, 2);
}
$('discover').addEventListener('click', () => {
  discovery = null; clearSelection(); $('model').replaceChildren(new Option('Discover models first', ''));
  $('runtime').textContent = 'No current runtime observation'; $('plugins').textContent = 'Plugin discovery pending.';
  invoke('/api/discover', {}, value => {
    discovery = value;
    $('runtime').textContent = `${value.endpoint}\n${value.build.component} ${value.build.package_version}\nService ${value.fence.service}`;
    $('plugins').textContent = value.plugins ? `Plugin manifests: ${value.plugins.map(p => p.name).join(' · ') || 'none'}. Availability is checked per model.` : 'Plugin inventory unavailable; see diagnostics.';
    $('model').replaceChildren(...(value.models.length ? value.models.map(m => new Option(m.id, m.id)) : [new Option('No served models', '')]));
    message(value.models.length ? 'Connected. Select a model and inspect its current capabilities.' : 'Connected, but no models are served. Serve a local model in Pumas and refresh.');
  });
});
$('model').addEventListener('change', () => { $('profile').value = ''; clearSelection(); });
$('profile').addEventListener('input', clearSelection);
$('inspect').addEventListener('click', () => {
  clearSelection();
  invoke('/api/inspect', { model: $('model').value, profile: $('profile').value, fence: discovery.fence }, value => {
    capabilities = value.capabilities; $('profile').value = capabilities.profile;
    message('Capabilities observed. Supported means advertised; inference remains untested until a request succeeds.');
    const first = capabilityRows(capabilities).find(row => row.support === 'supported'); if (first) choose(first.capability);
  });
});
$('run').addEventListener('click', () => {
  try {
    const request = JSON.parse($('request').value);
    if (request.model !== capabilities.model || request.profile !== capabilities.profile || request.capability !== selected) throw new Error('Inspect and select the requested model, profile and capability first.');
    request.request_id = crypto.randomUUID(); validateRequest(request); $('request').value = JSON.stringify(request, null, 2);
    $('output').textContent = 'Waiting for the actual runtime result…'; $('result-state').textContent = 'RUNNING';
    invoke('/api/run', { request, fence: discovery.fence }, async value => {
      await showResult(value); evidence.set(key(selected), 'SUCCEEDED'); message('Received and validated a correlated terminal result.');
    });
  } catch (error) { message(error.message, true); }
});
$('cancel').addEventListener('click', async () => {
  if (!active) return;
  $('cancel').disabled = true;
  try { const value = await api('/api/cancel', { id: active.id }); message(value.message ?? value.error?.message ?? 'Cancellation requested.', !!value.error); }
  catch { message('Cancellation could not be confirmed. The current request may still be running.', true); }
});
$('file').addEventListener('change', async () => {
  const file = $('file').files[0]; if (!file) return;
  const invocation = key(selected), original = $('request').value;
  try {
    if (file.size > LIMITS.fileBytes) throw new Error('Input file exceeds 8 MiB.');
    const data = new Uint8Array(await file.arrayBuffer());
    if (invocation !== key(selected) || original !== $('request').value || active) throw new Error('Selection changed while reading the file. Attach it again.');
    let binary = ''; for (let i = 0; i < data.length; i += 8192) binary += String.fromCharCode(...data.subarray(i, i + 8192));
    const request = JSON.parse(original);
    if (selected === 'image_to_text') {
      request.input.messages[0].content[1] = { kind: 'image', encoding: file.type === 'image/jpeg' ? 'jpeg' : 'png', data_base64: btoa(binary) };
    } else {
      if (/\.(wav|mp3|ogg|flac)$/i.test(file.name)) throw new Error('Expected raw PCM; convert containers explicitly first.');
      request.input.data_base64 = btoa(binary);
      request.input.sample_count = data.length / (request.input.channels * (request.input.encoding === 'pcm_f32le' ? 4 : 2));
    }
    validateRequest(request); $('request').value = JSON.stringify(request, null, 2); message(`Attached ${data.length} bytes. Review the request before running.`);
  } catch (error) { message(error.message, true); }
});
$('export').addEventListener('click', () => {
  const url = URL.createObjectURL(new Blob([JSON.stringify(receiptData, null, 2)], { type: 'application/json' }));
  const link = document.createElement('a'); link.href = url; link.download = 'pumas-playground-receipt.json'; link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
});
window.addEventListener('pagehide', () => { if (imageUrl) URL.revokeObjectURL(imageUrl); });
controls();
