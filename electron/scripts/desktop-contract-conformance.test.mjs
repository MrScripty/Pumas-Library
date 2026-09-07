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
