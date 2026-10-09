import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';

const base = {
  operation_id: 'c3f7d104-1234-4321-abcd-aaaaaaaaaaaa',
  endpoint: 'https://localhost:443', region: 'fixture', bucket: 'fixture',
  addressing: 'path', key: 'object', filename: 'weights.safetensors',
  sha256: '0'.repeat(64), family: 'fixture', official_name: 'Conditional fixture',
};

for (const consumer of ['electron', 'frontend']) {
  test(`${consumer}: conditional source facts stay closed and VersionId remains mandatory by default`, async () => {
    const source = await readFile(new URL(`../../${consumer}/src/generated/desktop-contract.validators.js`, import.meta.url));
    const wire = await import(`data:text/javascript;base64,${source.toString('base64')}`);
    const conditional = {...base, read_mode: 'conditional'};
    assert.equal(wire.validateS3ImportParams(conditional), true);
    assert.equal(wire.validateS3ImportParams({...base, version_id: 'v1'}), true);
    assert.equal(wire.validateS3ImportParams({...base, read_mode: 'version_id', version_id: 'v1'}), true);
    assert.equal(wire.validateS3ImportParams(base), false);
    for (const version_id of ['v1', '', 'null', null]) {
      assert.equal(wire.validateS3ImportParams({...conditional, version_id}), false);
      if (version_id !== 'v1') assert.equal(wire.validateS3ImportParams({...base, version_id}), false);
    }
    for (const override of [{read_mode: 'unknown'}, {sha256: ''}, {sha256: null},
      {key: '../escape'}, {filename: 'folder/CON.safetensors'}, {credentials: {}}, {allow_http: true}]) {
      assert.equal(wire.validateS3ImportParams({...conditional, ...override}), false);
    }
    const missingDigest = {...conditional};
    delete missingDigest.sha256;
    assert.equal(wire.validateS3ImportParams(missingDigest), false);
    const credentials = {access_key_id: 'fixture-access', secret_access_key: 'fixture-secret'};
    assert.equal(wire.validateS3AuthenticatedImportParams({source: conditional, credentials}), true);
    assert.equal(wire.validateS3AuthenticatedImportParams({source: {...conditional, version_id: 'v1'}, credentials}), false);
    const common = {...base};
    for (const key of ['key', 'filename', 'sha256']) delete common[key];
    const bundle = {...common, primary_logical_path: 'weights.safetensors', files: [
      {key: 'a', version_id: 'v1', logical_path: 'weights.safetensors', sha256: base.sha256},
      {key: 'b', version_id: 'v2', logical_path: 'config.json', sha256: base.sha256},
    ]};
    assert.equal(wire.validateS3BundleImportParams(bundle), true);
    assert.equal(wire.validateS3BundleImportParams({...bundle, read_mode: 'conditional'}), false);
    const discovery = {...common, prefix: '', timeout_ms: 1000};
    delete discovery.family;
    delete discovery.official_name;
    assert.equal(wire.validateS3DiscoveryParams(discovery), true);
    assert.equal(wire.validateS3DiscoveryParams({...discovery, read_mode: 'conditional'}), false);
  });
}
