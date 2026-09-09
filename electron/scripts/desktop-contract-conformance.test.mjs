import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { fileURLToPath, URL } from 'node:url';
import test from 'node:test';
import { build } from 'esbuild';

const fixturePath = process.env.PUMAS_DESKTOP_CONTRACT_FIXTURES;
if (!fixturePath) throw new Error('Run this integration test through with-desktop-contract-fixtures.mjs.');
const fixtures = JSON.parse(await readFile(fixturePath, 'utf8'));
const compiled = await build({entryPoints:[fileURLToPath(new URL('../src/generated/desktop-contract.ts', import.meta.url))], bundle:true, format:'esm', platform:'browser', write:false});
const contract = await import(`data:text/javascript;base64,${Buffer.from(compiled.outputFiles[0].text).toString('base64')}`);

test('library metadata decoding preserves actual producer optionality and nested JSON', () => {
  for (const name of ['library_model_metadata', 'library_model_metadata_empty', 'library_model_metadata_gguf']) {
    const result = contract.decodeLibraryModelMetadataOutcome(fixtures[name]);
    assert.equal(result.status, 'valid', name);
    assert.deepEqual(JSON.parse(JSON.stringify(result.value)), fixtures[name]);
  }
  const valid = fixtures.library_model_metadata;
  for (const patch of [
    { success: false }, { extra: true }, { model_id: 42 },
    { stored_metadata: null }, { stored_metadata: [] }, { effective_metadata: 'text' },
    { embedded_metadata: null }, { embedded_metadata: { file_type: 'gguf', metadata: 42 } },
    { embedded_metadata: { file_type: 'gguf' } }, { embedded_metadata: { metadata: {} } },
    { stored_metadata: { nested: [-9007199254740992] } },
    { primary_file: null }, { component_manifest: null },
    { component_manifest: [{ ...valid.component_manifest[0], state: 'unknown' }] },
  ]) assert.equal(contract.decodeLibraryModelMetadataOutcome({ ...valid, ...patch }).status, 'invalid', JSON.stringify(patch));
  for (const key of ['source_library', 'class_name']) {
    const incomplete = { ...valid.component_manifest[0] };
    delete incomplete[key];
    assert.equal(contract.decodeLibraryModelMetadataOutcome({ ...valid, component_manifest: [incomplete] }).status, 'invalid', key);
  }
});

test('inference settings preserve actual producer values and reject malformed nested facts', () => {
  for (const name of ['inference_settings', 'inference_settings_empty']) {
    const result = contract.decodeInferenceSettingsOutcome(fixtures[name]);
    assert.equal(result.status, 'valid', name);
    assert.deepEqual(JSON.parse(JSON.stringify(result.value)), fixtures[name]);
  }
  const valid = fixtures.inference_settings;
  const first = valid.inference_settings[0];
  for (const patch of [
    { param_type: 'Float' }, { description: 42 }, { key: null },
    { constraints: { min: null, max: null } },
    { constraints: { min: '1', max: null, allowed_values: null } },
    { default: { nested: [-9007199254740992] } },
    { constraints: { min: null, max: null, allowed_values: [{ nested: 9007199254740992 }] } },
    { extra: true },
  ]) {
    assert.equal(contract.decodeInferenceSettingsOutcome({ ...valid, inference_settings: [{ ...first, ...patch }] }).status, 'invalid', JSON.stringify(patch));
  }
  for (const key of ['default', 'description', 'constraints']) {
    const incomplete = { ...first };
    delete incomplete[key];
    assert.equal(contract.decodeInferenceSettingsOutcome({ ...valid, inference_settings: [incomplete] }).status, 'invalid', key);
  }
});

test('HF download-details request decoding agrees with the actual Rust parser', () => {
  const probes = fixtures.hf_download_details_request_probes;
  assert.ok(Array.isArray(probes) && probes.length > 0);
  for (const { params, accepted } of probes) {
    const result = contract.decodeGetHfDownloadDetailsParams(params);
    assert.equal(result.status, accepted ? 'valid' : 'invalid', JSON.stringify(params));
    if (accepted) assert.deepEqual(JSON.parse(JSON.stringify(result.value)), params);
  }
});

test('HF download details preserve producer identities, groups and unknown sizes', () => {
  for (const name of ['hf_download_details_success', 'hf_download_details_empty', 'hf_download_details_failure']) {
    const result = contract.decodeHfDownloadDetailsOutcome(fixtures[name]);
    assert.equal(result.status, 'valid', name);
    assert.deepEqual(JSON.parse(JSON.stringify(result.value)), fixtures[name]);
    assert.ok(Object.isFrozen(result.value));
    if (result.value.success) assert.ok(Object.isFrozen(result.value.details.downloadOptions));
  }
});

test('HF download-details decoder rejects malformed nested facts and ambiguous outcomes', () => {
  const valid = fixtures.hf_download_details_success;
  const option = { quant: 'Q4_K_M', sizeBytes: null };
  for (const patch of [
    { repoId: 42 }, { totalSizeBytes: -1 }, { totalSizeBytes: 0.5 },
    { totalSizeBytes: 9007199254740992 }, { downloadOptions: null }, { extra: true },
    ...[{ sizeBytes: -1 }, { sizeBytes: 9007199254740992 }, { quant: 42 }, { extra: true },
      { fileGroup: { filenames: [123], shardCount: 1, label: 'weights' } },
      { fileGroup: { filenames: ['weights'], shardCount: -1, label: 'weights' } },
      { fileGroup: { filenames: ['weights'], shardCount: 4294967296, label: 'weights' } },
      { fileGroup: { filenames: ['weights'], shardCount: 1, label: 'weights', extra: true } },
    ].map(change => ({ downloadOptions: [{ ...option, ...change }] })),
  ]) assert.equal(contract.decodeHfDownloadDetailsOutcome({ ...valid, details: { ...valid.details, ...patch } }).status, 'invalid', JSON.stringify(patch));
  for (const response of [
    { success: true }, { success: false }, { ...valid, success: false },
    { ...valid, error: 'unexpected' }, { success: false, error: 42 },
  ]) assert.equal(contract.decodeHfDownloadDetailsOutcome(response).status, 'invalid');
  const missingSize = structuredClone(valid);
  delete missingSize.details.totalSizeBytes;
  assert.equal(contract.decodeHfDownloadDetailsOutcome(missingSize).status, 'invalid');
});

test('registered-link health preserves actual producer facts and rejects contradictory reports', () => {
  for (const name of ['link_health_healthy','link_health_degraded']) {
    const outcome = contract.decodeLinkHealthOutcome(fixtures[name]);
    assert.equal(outcome.status, 'valid', name);
    assert.equal(outcome.value.total_links, outcome.value.healthy_links + outcome.value.broken_links.length);
    assert.equal(Object.isFrozen(outcome.value.broken_links), true);
  }
  for (const change of [
    {status:'unknown'}, {success:false}, {error:'private failure'},
    {healthy_links:-1}, {total_links:9007199254740992}, {total_links:0.5},
    {total_links:99}, {status:'healthy'}, {broken_links:[42]}, {extra:true},
  ]) {
    assert.equal(contract.decodeLinkHealthOutcome({...fixtures.link_health_degraded,...change}).status, 'invalid', JSON.stringify(change));
  }
});

test('conversion reads preserve producer camelCase, nullable fields and complete enum vocabulary', () => {
  for (const response of fixtures.conversion_progress) {
    const decoded = contract.decodeConversionProgressResponse(response);
    assert.equal(decoded.status, 'valid');
    assert.deepEqual(JSON.parse(JSON.stringify(decoded.value)), response);
    assert.ok(Object.isFrozen(decoded.value.progress));
    assert.equal('conversion_id' in decoded.value.progress, false);
    assert.equal(typeof decoded.value.progress.conversionId, 'string');
    assert.equal('pipelineStepLabel' in decoded.value.progress, true);
  }
  assert.deepEqual(JSON.parse(JSON.stringify(contract.decodeConversionProgressResponse(fixtures.conversion_missing).value)), {success:true,progress:null});
  assert.deepEqual(JSON.parse(JSON.stringify(contract.decodeConversionListOutcome(fixtures.conversion_list).value)), fixtures.conversion_list);
});

test('conversion decoder rejects old field names and unsafe numeric evidence', () => {
  const valid = fixtures.conversion_progress[0];
  for (const patch of [{progress:-0.1},{progress:1.1},{bytesWritten:9007199254740992},
    {tensorsCompleted:-1},{tensorsTotal:0.5},{status:'unknown'},{direction:'unknown'},
    {conversion_id:'legacy'},{extra:true},{error:'/private/diagnostic'},
  ]) assert.equal(contract.decodeConversionProgressResponse({...valid,progress:{...valid.progress,...patch}}).status, 'invalid');
  const missingNull = structuredClone(valid);
  delete missingNull.progress.currentTensor;
  assert.equal(contract.decodeConversionProgressResponse(missingNull).status, 'invalid');
  assert.equal(contract.decodeConversionProgressResponse({success:false,progress:null}).status, 'invalid');
  assert.equal(contract.decodeConversionListOutcome({success:true,conversions:[{...valid.progress,progress:2}]}).status, 'invalid');
});

test('setup snapshots preserve all producer states, identities and explicit idle', () => {
  for (const [name,decode] of [['conversion_setup_started',contract.decodeConversionSetupStartedOutcome],['conversion_setup_status',contract.decodeConversionSetupStatusOutcome]]) {
    for (const response of fixtures[name]) {
      const result = decode(response);
      assert.equal(result.status, 'valid');
      assert.deepEqual(JSON.parse(JSON.stringify(result.value)), response);
      assert.equal(Object.isFrozen(result.value.setup), true);
      assert.equal(JSON.stringify(result.value).includes('/secret/'), false);
    }
  }
  assert.deepEqual(JSON.parse(JSON.stringify(contract.decodeConversionSetupStatusOutcome(fixtures.conversion_setup_idle).value)), {success:true,setup:null});
  assert.equal(contract.decodeConversionSetupStartedOutcome(fixtures.conversion_setup_idle).status, 'invalid');
});

test('setup decoders reject contradictory status, malformed identity and raw diagnostics', () => {
  const active = fixtures.conversion_setup_started[0];
  for (const patch of [{operationId:''}, {operationId:'old-id'}, {operationId:active.setup.operationId.toUpperCase()},
    {operationId:`${active.setup.operationId}\n`}, {operation_id:active.setup.operationId}, {status:'ready'},
    {status:'failed',error:null}, {error:'Conversion environment setup did not complete successfully.'},
    {status:'failed',error:'/private/diagnostic'}, {extra:true},
  ]) assert.equal(contract.decodeConversionSetupStartedOutcome({...active,setup:{...active.setup,...patch}}).status, 'invalid');
  const missing = structuredClone(active);
  delete missing.setup.error;
  assert.equal(contract.decodeConversionSetupStartedOutcome(missing).status, 'invalid');
  assert.equal(contract.decodeConversionSetupStatusOutcome({success:false,setup:null}).status, 'invalid');
  for (const params of [{}, {expected_previous_operation_id:null}, {expected_previous_operation_id:active.setup.operationId}]) {
    assert.equal(contract.decodeStartConversionSetupParams(params).status, 'valid');
  }
  for (const params of [{force:true}, {expectedPreviousOperationId:active.setup.operationId}, {expected_previous_operation_id:''}, {expected_previous_operation_id:42}]) {
    assert.equal(contract.decodeStartConversionSetupParams(params).status, 'invalid');
  }
});

test('backend setup request decoders preserve the actual RPC parser contract', () => {
  const probes = fixtures.backend_setup_request_probes;
  assert.ok(Array.isArray(probes) && probes.length > 0);
  for (const {method, params, accepted} of probes) {
    assert.ok(['start_backend_setup', 'get_backend_setup'].includes(method));
    const decode = method === 'start_backend_setup'
      ? contract.decodeStartBackendSetupParams
      : contract.decodeGetBackendSetupParams;
    const result = decode(params);
    assert.equal(result.status, accepted ? 'valid' : 'invalid', JSON.stringify({method,params}));
    if (accepted) {
      assert.deepEqual(JSON.parse(JSON.stringify(result.value)), params);
      assert.ok(Object.isFrozen(result.value));
    }
  }
});

test('conversion operation outcomes preserve producer readiness, cancellation and quant metadata', () => {
  const pairs = [
    ['conversion_started',contract.decodeConversionStartedOutcome],
    ['conversion_setup_success',contract.decodeSuccessOutcome],
    ['conversion_quant_types',contract.decodeSupportedQuantTypesOutcome],
    ['conversion_quant_types_nullable_backend',contract.decodeSupportedQuantTypesOutcome],
    ['conversion_backend_status',contract.decodeBackendStatusOutcome],
  ];
  for (const [name,decode] of pairs) {
    const result = decode(fixtures[name]);
    assert.equal(result.status,'valid',name);
    assert.deepEqual(JSON.parse(JSON.stringify(result.value)),fixtures[name]);
  }
  for (const result of fixtures.conversion_cancelled) assert.equal(contract.decodeConversionCancelledOutcome(result).value.cancelled,result.cancelled);
  for (const result of fixtures.conversion_environment) assert.equal(contract.decodeConversionEnvironmentOutcome(result).value.ready,result.ready);
  const option = contract.decodeSupportedQuantTypesOutcome(fixtures.conversion_quant_types).value.quant_types[0];
  assert.equal(typeof option.bitsPerWeight,'number');
  assert.equal(typeof option.imatrixRecommended,'boolean');
  assert.equal('backend' in option,true);
  assert.equal(Object.isFrozen(option),true);
});

test('conversion operation decoders reject malformed outcomes rather than coercing success', () => {
  for (const value of [{success:false},{success:true,extra:true}]) assert.equal(contract.decodeSuccessOutcome(value).status,'invalid');
  for (const value of [{success:true,ready:null},{success:true,ready:'false'}]) assert.equal(contract.decodeConversionEnvironmentOutcome(value).status,'invalid');
  assert.equal(contract.decodeConversionCancelledOutcome({success:true,cancelled:0}).status,'invalid');
  for (const conversion_id of ['', '   ', '\u0085', 'x'.repeat(4097), 'é'.repeat(2049)]) assert.equal(contract.decodeConversionStartedOutcome({success:true,conversion_id}).status,'invalid');
  for (const conversion_id of [' conversion ', '\uFEFF', 'é'.repeat(2048)]) assert.equal(contract.decodeConversionStartedOutcome({success:true,conversion_id}).status,'valid');
  const valid = fixtures.conversion_quant_types.quant_types[0];
  for (const patch of [{bitsPerWeight:-1},{bitsPerWeight:NaN},{bitsPerWeight:Infinity},{backend:'unknown'},{imatrixRecommended:null},{bits_per_weight:4}]) {
    assert.equal(contract.decodeSupportedQuantTypesOutcome({success:true,quant_types:[{...valid,...patch}]}).status,'invalid');
  }
  const missing = {...valid}; delete missing.backend;
  assert.equal(contract.decodeSupportedQuantTypesOutcome({success:true,quant_types:[missing]}).status,'invalid');
  assert.equal(contract.decodeBackendStatusOutcome({success:true,backends:[{backend:'unknown',name:'unknown',ready:true}]}).status,'invalid');
});

test('actual producer catalog and FTS cross the generated decoder', () => {
  const models = contract.decodeModelsOutcome(fixtures.models);
  const search = contract.decodeCatalogSearchOutcome(fixtures.search);
  assert.equal(models.status, 'valid');
  assert.equal(search.status, 'valid');
  assert.equal(Object.isFrozen(models.value.models), true);
  for (const model of search.value.models) assert.deepEqual(models.value.models[model.id], model);
  assert.deepEqual(search.value.models.map(model => model.id), ['llm/example/complete','llm/example/partial','llm/example/duplicate','llm/example/duplicate-peer']);
});

test('producer-impossible omitted null and missing explicit-null fields reject', () => {
  const changed = structuredClone(fixtures.models);
  changed.models['llm/example/complete'].format = null;
  assert.equal(contract.decodeModelsOutcome(changed).status, 'invalid');
  assert.equal(contract.decodePartialDownloadOutcome({success:true,action:'resume',download_id:'id',status:'queued',reason_code:null}).status, 'invalid');
});

test('map identity, duplicate consistency and partial progress cannot be weakened', () => {
  for (const mutate of [
    value => { value.models['llm/example/complete'].id = 'other'; },
    value => { value.models['llm/example/duplicate'].integrity.count = 9; },
    value => { value.models['llm/example/partial'].artifact.downloadProgressFraction = 1; },
    value => { value.models['llm/example/partial'].artifact.reasons = []; },
    value => { value.models['llm/example/complete'].sizeBytes = 9007199254740992; },
    value => { value.models['llm/example/complete'].displayName = '   '; },
    value => { value.models['llm/example/partial'].artifact.recovery.selectedArtifactId = ' '; },
    value => { value.models['llm/example/partial'].artifact.recovery.selectedArtifactId = 'é'.repeat(2049); },
    value => { value.models['llm/example/partial'].artifact.recovery.repoId = 'owner/..'; },
  ]) {
    const changed = structuredClone(fixtures.models);
    mutate(changed);
    assert.equal(contract.decodeModelsOutcome(changed).status, 'invalid');
  }
});

test('decoder retains no mutable alias and refuses non-JSON effects', () => {
  const changed = structuredClone(fixtures.models);
  const accepted = contract.decodeModelsOutcome(changed);
  changed.models['llm/example/complete'].displayName = 'mutated';
  assert.equal(accepted.value.models['llm/example/complete'].displayName, 'complete');
  let calls = 0;
  assert.equal(contract.decodeModelsOutcome({get success() {calls++; return true;}}).status, 'invalid');
  assert.equal(calls, 0);
});

test('actual download and recovery outcomes preserve nulls, numeric bounds and action correlations', () => {
  for (const [name, value] of [
    ['DownloadStatusOutcome',fixtures.download_status], ['DownloadListOutcome',fixtures.download_list],
    ['DownloadStartedOutcome',fixtures.download_started], ['DownloadMutationOutcome',fixtures.download_mutation],
    ['PartialDownloadOutcome',fixtures.recovery_outcome], ['RecoverDownloadParams',fixtures.recovery_request],
    ['PartialDownloadOutcome',fixtures.recovery_busy_outcome],
  ]) assert.equal(contract[`decode${name}`](value).status, 'valid', name);
  for (const [key, value] of [['progress',1.01], ['downloadedBytes',9007199254740992], ['speed',-1], ['etaSeconds',Number.POSITIVE_INFINITY]]) {
    assert.equal(contract.decodeDownloadStatusOutcome({...fixtures.download_status,[key]:value}).status, 'invalid');
  }
  assert.equal(contract.decodePartialDownloadOutcome({...fixtures.recovery_outcome,status:'paused'}).status, 'invalid');
  const busy = contract.decodePartialDownloadOutcome(fixtures.recovery_busy_outcome);
  assert.equal(busy.value.reason_code, 'download_root_busy');
  assert.equal(busy.value.success, false);
  assert.equal(busy.value.download_id, null);
  assert.equal(contract.decodePartialDownloadOutcome({...fixtures.recovery_busy_outcome,download_id:'unexpected-admission',status:'queued'}).status, 'invalid');
  assert.equal(contract.decodeDownloadMutationOutcome({success:false}).status, 'invalid');
  assert.equal(contract.decodeRecoverDownloadParams({...fixtures.recovery_request,modelId:'../escape'}).status, 'invalid');
  assert.equal(contract.decodeSearchCatalogParams({query:'',offset:4294967296}).status, 'invalid');
  assert.equal(contract.decodeDownloadStatusOutcome({...fixtures.download_status,retryLimit:4294967296}).status, 'invalid');
  for (const probe of fixtures.recovery_request_probes) assert.equal(contract.decodeRecoverDownloadParams(probe.request).status === 'valid', probe.accepted);
  for (const probe of fixtures.catalog_text_probes) {
    const changed = structuredClone(fixtures.models);
    changed.models['llm/example/complete'].displayName = probe.input;
    assert.equal(contract.decodeModelsOutcome(changed).status === 'valid', probe.input === probe.emitted, JSON.stringify(probe.input));
  }
});
