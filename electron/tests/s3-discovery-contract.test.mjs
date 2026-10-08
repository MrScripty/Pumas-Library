import assert from 'node:assert/strict';
import test from 'node:test';
import {validateApiCallPayload} from '../dist/ipc-validation.js';
import {decodeS3DiscoveryOutcome} from '../dist/generated/desktop-contract.js';
import {decodeS3ImportRpcResult,isS3ImportMethod} from '../dist/s3-import-rpc.js';
const id='c3f7d104-1234-4321-abcd-aaaaaaaaaaaa';
const source={operation_id:id,endpoint:'https://source.invalid',region:'fixture-region',bucket:'fixture-bucket',addressing:'path',prefix:'models/',timeout_ms:15000};
test('discovery admits only closed bounded source requests through the actual IPC receiver',()=>{
  assert.deepEqual(JSON.parse(JSON.stringify(validateApiCallPayload('start_s3_prefix_discovery',source).params)),source);
  for(const patch of [{timeout_ms:0},{timeout_ms:30001},{prefix:'x'.repeat(1025)},{prefix:'bad\n'},{endpoint:'http://source.invalid'},{allow_http:true},{secret_access_key:'synthetic-secret'}])
    assert.throws(()=>validateApiCallPayload('start_s3_prefix_discovery',{...source,...patch}),e=>!e.message.includes('synthetic-secret'));
  for(const method of ['start_s3_prefix_discovery','start_authenticated_s3_prefix_discovery','get_s3_prefix_discovery','cancel_s3_prefix_discovery']) assert.ok(isS3ImportMethod(method));
  const credentials={access_key_id:'synthetic-key',secret_access_key:'synthetic-secret',session_token:null};
  assert.ok(validateApiCallPayload('start_authenticated_s3_prefix_discovery',{source,credentials}));
  assert.throws(()=>validateApiCallPayload('start_authenticated_s3_prefix_discovery',{source,credentials:{...credentials,profile:'ambient'}}));
});
test('discovery projects complete pin observations and rejects partial or credential-bearing responses',()=>{
  const object={key:'models/weights.gguf',version_id:'exact-v1',etag:'"opaque"',size_bytes:'18446744073709551615'};
  const complete={status:'complete',operation_id:id,pages:1,objects:[object]};
  assert.deepEqual(JSON.parse(JSON.stringify(decodeS3ImportRpcResult('get_s3_prefix_discovery',complete))),complete);
  for(const invalid of [{status:'unavailable',secret_access_key:'synthetic-secret'}, {...complete,credentials:'synthetic-secret'},
    {...complete,objects:[{...object,session_token:'synthetic-secret'}]}, {...complete,pages:9}, {...complete,objects:Array(33).fill(object)},
    {...complete,objects:[{...object,etag:'x'.repeat(1025)}]}, {status:'incomplete',operation_id:id,objects:[object]}]) {
    assert.notEqual(decodeS3DiscoveryOutcome(invalid).status,'valid');
    assert.throws(()=>decodeS3ImportRpcResult('get_s3_prefix_discovery',invalid),e=>!e.message.includes('synthetic-secret'));
  }
  for(const status of ['running','incomplete','cancelled','deadline','not_found']) assert.equal(decodeS3DiscoveryOutcome({status,operation_id:id}).status,'valid');
});
