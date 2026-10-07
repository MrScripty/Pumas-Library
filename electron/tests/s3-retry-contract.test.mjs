import assert from 'node:assert/strict';
import test from 'node:test';
import { createRequire } from 'node:module';
const require=createRequire(import.meta.url);
const {decodeS3ImportRpcResult,isS3ImportMethod}=require('../dist/s3-import-rpc.js');
const {validateApiCallPayload}=require('../dist/ipc-validation.js');
const id='c3f7d104-1234-4321-abcd-aaaaaaaaaaaa';
test('retry and inspection are registered and validate closed requests at the privileged receiver',()=>{
  for (const operation_id of [id, null]) {
    const request={operation_id};
    const result=validateApiCallPayload('get_s3_transfer_retry',request);
    assert.deepEqual(JSON.parse(JSON.stringify(result.params)),request);
    assert.notEqual(result.params,request);
  }
  for (const credentials of [null,{access_key_id:'fixture-key',secret_access_key:'fixture-secret',session_token:null}]) {
    const request={operation_id:id,credentials};
    const result=validateApiCallPayload('retry_s3_model_transfer',request);
    assert.deepEqual(JSON.parse(JSON.stringify(result.params)),request);
    assert.ok(Object.isFrozen(result.params));
    assert.notEqual(result.params,request);
  }
  for (const patch of [{operation_id:null},{operation_id:id.toUpperCase()},{endpoint:'https://replacement.invalid'},
    {files:[]},{credentials:{access_key_id:'fixture-key',secret_access_key:''}},
    {credentials:{access_key_id:'fixture-key',secret_access_key:'fixture-secret',profile:'saved'}}]) {
    assert.throws(()=>validateApiCallPayload('retry_s3_model_transfer',{operation_id:id,credentials:null,...patch}),
      error=>error.message==='Invalid S3 retry parameters');
  }
  assert.throws(()=>validateApiCallPayload('get_s3_transfer_retry',{operation_id:id,credentials:{}}));
  for (const empty of [undefined,null,{}]) assert.deepEqual(JSON.parse(JSON.stringify(validateApiCallPayload('inspect_persisted_s3_imports',empty).params)),{});
  for (const invalid of [[],{operation_id:id},{retry:true},{[Symbol('hidden')]:true}]) {
    assert.throws(()=>validateApiCallPayload('inspect_persisted_s3_imports',invalid),/Invalid S3 inspection parameters/);
  }
});
test('retry receiving boundary exposes only closed identity/availability and original import outcomes',()=>{
  for(const name of ['get_s3_transfer_retry','retry_s3_model_transfer']) assert.equal(isS3ImportMethod(name),true);
  for(const value of [{status:'ready',operation_id:id,authentication_required:true},{status:'unavailable',reason:'no_live_custody'}]) assert.deepEqual(JSON.parse(JSON.stringify(decodeS3ImportRpcResult('get_s3_transfer_retry',value))),value);
  for(const value of [{status:'ready',operation_id:id,authentication_required:'yes'},{status:'ready',operation_id:id,authentication_required:false,secret:'synthetic'},{status:'unavailable',reason:'automatic_resume'}]) assert.throws(()=>decodeS3ImportRpcResult('get_s3_transfer_retry',value),/S3 import RPC is unavailable/);
  const running={status:'running',operation_id:id,progress:{phase:'pending',downloaded_for_current_file:'0'}};
  assert.deepEqual(JSON.parse(JSON.stringify(decodeS3ImportRpcResult('retry_s3_model_transfer',running))),running);
});
